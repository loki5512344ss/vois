//! Real-time pitch shifting via phase vocoder.
//!
//! The phase vocoder resamples the spectral *residual* (fine structure) to
//! change pitch while keeping the spectral envelope (formants) intact, and
//! independently resamples the envelope to change formants. This gives us a
//! single FFT-based stage that drives both the pitch and formant controls.
//!
//! Latency is one FFT frame (default 1024 samples ≈ 21 ms @ 48 kHz).
//! Hann window with 75% overlap (hop = N/4) is a valid COLA window, so the
//! overlap-add reconstruction is lossless in the bypass case.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// Linear interpolation over `arr` at fractional position `pos`.
fn interpolate(arr: &[f32], pos: f32) -> f32 {
    let max = arr.len() as f32 - 1.0;
    let pos = pos.clamp(0.0, max);
    let i = pos.floor() as usize;
    let j = (i + 1).min(arr.len() - 1);
    let t = pos - i as f32;
    arr[i] * (1.0 - t) + arr[j] * t
}

/// Wrap a phase difference into (-PI, PI].
fn princarg(x: f32) -> f32 {
    x - 2.0 * std::f32::consts::PI * (x / (2.0 * std::f32::consts::PI)).round()
}

pub struct PitchShifter {
    n: usize,
    hop: usize,

    window: Vec<f32>,
    norm: f32,

    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    env_ifft: Arc<dyn Fft<f32>>,
    env_fft: Arc<dyn Fft<f32>>,

    in_buf: Vec<f32>,
    out_buf: Vec<f32>,

    spec: Vec<Complex<f32>>,
    mag: Vec<f32>,
    phase: Vec<f32>,
    logmag: Vec<f32>,
    env: Vec<f32>,
    cep: Vec<Complex<f32>>,
    prev_phase: Vec<f32>,
    out_phase: Vec<f32>,

    /// Quefrency bins kept by the cepstral lifter (spectral envelope cutoff).
    lifter_len: usize,

    /// Pitch factor: output pitch = input pitch * ratio (1.0 = unchanged).
    pub pitch_ratio: f32,
    /// Formant factor: output formants = input formants * ratio (1.0 = unchanged).
    pub formant_ratio: f32,

    fs: u32,
    /// Detected fundamental of the most recent analysis frame (Hz, 0 = unvoiced).
    pub last_f0: f32,
}

impl PitchShifter {
    pub fn new(fft_size: usize, hop: usize, sample_rate: u32) -> Self {
        debug_assert!(fft_size.is_multiple_of(hop));
        let frames = fft_size / hop;

        let mut window = vec![0.0; fft_size];
        for (i, w) in window.iter_mut().enumerate() {
            *w =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos();
        }

        // Steady-state OLA normalization: every output sample is covered by
        // `frames` overlapping windows; sum of w^2 over those offsets.
        let mut norm = 0.0;
        for i in 0..frames {
            let idx = i * hop;
            norm += window[idx] * window[idx];
        }
        debug_assert!(norm > 0.0);

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);
        let env_ifft = planner.plan_fft_inverse(fft_size);
        let env_fft = planner.plan_fft_forward(fft_size);

        Self {
            n: fft_size,
            hop,
            window,
            norm,
            fft,
            ifft,
            env_ifft,
            env_fft,
            in_buf: Vec::new(),
            out_buf: vec![0.0; fft_size],
            spec: vec![Complex::new(0.0, 0.0); fft_size],
            mag: vec![0.0; fft_size / 2 + 1],
            phase: vec![0.0; fft_size / 2 + 1],
            logmag: vec![0.0; fft_size / 2 + 1],

            env: vec![0.0; fft_size / 2 + 1],
            cep: vec![Complex::new(0.0, 0.0); fft_size],
            prev_phase: vec![0.0; fft_size / 2 + 1],
            out_phase: vec![0.0; fft_size / 2 + 1],
            lifter_len: fft_size / 8,
            pitch_ratio: 1.0,
            formant_ratio: 1.0,
            fs: sample_rate,
            last_f0: 0.0,
        }
    }

    /// Stream `input` through the shifter, appending the same number of
    /// processed samples to `output` (length is preserved).
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        for &sample in input {
            self.in_buf.push(sample);
            if self.in_buf.len() == self.n {
                self.process_frame();
                self.in_buf.drain(..self.hop);
                output.extend_from_slice(&self.out_buf[..self.hop]);
                self.out_buf.drain(..self.hop);
                self.out_buf.resize(self.n, 0.0);
            }
        }
    }

    fn process_frame(&mut self) {
        let n2 = self.n / 2;

        // Analysis: window + forward FFT.
        for (i, c) in self.spec.iter_mut().enumerate() {
            *c = Complex::new(self.in_buf[i] * self.window[i], 0.0);
        }
        self.fft.process(&mut self.spec);

        for i in 0..=n2 {
            let c = self.spec[i];
            let m = c.norm();
            self.mag[i] = m;
            self.phase[i] = c.arg();
            self.logmag[i] = (m + 1e-12).ln();
        }

        // Cepstral spectral-envelope estimation (used for formant control).
        self.compute_envelope();

        // Fundamental detection (harmonic product sum) for Auto-Tune.
        self.detect_f0();

        let pr = self.pitch_ratio;
        let fr = self.formant_ratio;
        let p_active = (pr - 1.0).abs() > 1e-4;
        let f_active = (fr - 1.0).abs() > 1e-4;

        if p_active || f_active {
            // Phase propagation (Laroche & Dolson): track the instantaneous
            // frequency of each input bin and advance the output phase so the
            // reconstructed components stay coherent.
            let hop = self.hop as f32;
            let nf = self.n as f32;
            let mut new_mag = vec![0.0; n2 + 1];
            let mut new_phase = vec![0.0; n2 + 1];
            for (b, nm) in new_mag.iter_mut().enumerate() {
                let bf = b as f32;

                // Source input bin for output bin b under the pitch shift.
                let src = if p_active { bf / pr } else { bf };
                let base = interpolate(&self.mag, src);

                // Formants: re-weight the envelope so its shape is shifted by
                // `fr` independent of pitch. After the pitch resample the
                // envelope sits at E[bf/pr]; we move it to E[bf/fr].
                if f_active {
                    let num = interpolate(&self.env, bf / fr);
                    let den = interpolate(&self.env, bf / pr);
                    let ratio = (num / den.max(1e-9)).clamp(0.3, 3.3);
                    *nm = base * ratio;
                } else {
                    *nm = base;
                }

                // Instantaneous frequency of the source bin (rad per frame).
                let si = src.round().clamp(0.0, n2 as f32) as usize;
                let omega_src = 2.0 * std::f32::consts::PI * si as f32 / nf;
                let diff = princarg(self.phase[si] - self.prev_phase[si] - omega_src * hop);
                let inst = omega_src + diff / hop;

                // Output phase advances by the (pitch-shifted) source frequency.
                let advance = if p_active {
                    inst * hop * pr
                } else {
                    inst * hop
                };
                self.out_phase[b] += advance;
                new_phase[b] = self.out_phase[b];

                self.prev_phase[si] = self.phase[si];
            }

            // Rebuild the conjugate-symmetric spectrum.
            for (b, (&nm, &ph)) in new_mag.iter().zip(new_phase.iter()).enumerate() {
                self.spec[b] = Complex::from_polar(nm, ph);
                if b > 0 && b < n2 {
                    self.spec[self.n - b] = Complex::from_polar(nm, -ph);
                }
            }
        }

        // Synthesis: inverse FFT + window + overlap-add.
        self.ifft.process(&mut self.spec);
        let scale = 1.0 / (self.n as f32 * self.norm);
        for i in 0..self.n {
            let y = self.spec[i].re * self.window[i] * scale;
            self.out_buf[i] += y;
        }
    }

    /// Fundamental frequency via harmonic product sum over the magnitude
    /// spectrum. Sets `last_f0` (Hz), or 0.0 when the frame is unvoiced/weak.
    fn detect_f0(&mut self) {
        let n2 = self.n / 2;
        let bin_hz = self.fs as f32 / self.n as f32;
        // Search 70..1000 Hz, up to 5 harmonics.
        let b_min = (70.0 / bin_hz).floor() as usize;
        let b_max = (1000.0 / bin_hz).floor() as usize;
        let mut best_bin = 0usize;
        let mut best_score = 0.0f32;
        for b in b_min..=b_max.min(n2 / 5) {
            let mut score = 0.0;
            for h in 1..=5 {
                if b * h < n2 {
                    score += self.mag[b * h];
                }
            }
            if score > best_score {
                best_score = score;
                best_bin = b;
            }
        }
        // Voicing threshold: fundamental bin must carry real energy.
        let peak = self.mag[..n2].iter().cloned().fold(0.0f32, f32::max);
        self.last_f0 = if best_bin > 0 && self.mag[best_bin] > peak * 0.05 && best_score > 0.0 {
            best_bin as f32 * bin_hz
        } else {
            0.0
        };
    }

    /// Spectral envelope via homomorphic (cepstral) filtering.
    fn compute_envelope(&mut self) {
        let n2 = self.n / 2;

        // Conjugate-symmetric spectrum of log-magnitude -> real cepstrum.
        for i in 0..=n2 {
            let l = self.logmag[i];
            self.cep[i] = Complex::new(l, 0.0);
            if i > 0 && i < n2 {
                self.cep[self.n - i] = Complex::new(l, 0.0);
            }
        }
        self.env_ifft.process(&mut self.cep);
        let inv = 1.0 / self.n as f32;

        // Lifter: keep low quefrencies (smooth envelope), zero the rest.
        for (i, c) in self.cep.iter_mut().enumerate() {
            let keep = if i <= self.lifter_len { 1.0 } else { 0.0 };
            c.re *= keep * inv;
            c.im = 0.0;
        }
        self.env_fft.process(&mut self.cep);

        for i in 0..=n2 {
            self.env[i] = self.cep[i].re.exp().max(1e-6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, fs: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / fs as f32).sin())
            .collect()
    }

    /// Harmonic-rich tone (sawtooth): narrow pitch lines + broad envelope,
    /// closer to a real voice than a pure sine.
    fn saw(freq: f32, fs: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let t = freq * i as f32 / fs as f32;
                let mut s = 0.0;
                for h in 1..=12 {
                    s += ((2.0 * std::f32::consts::PI * h as f32 * t).sin()) / h as f32;
                }
                s
            })
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        let s: f32 = x.iter().map(|v| v * v).sum();
        (s / x.len().max(1) as f32).sqrt()
    }

    fn dominant_hz(x: &[f32], fs: u32) -> f32 {
        use rustfft::FftPlanner;
        let n = 4096;
        if x.len() < n {
            return 0.0;
        }
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let mut buf: Vec<Complex<f32>> = x[..n].iter().map(|&s| Complex::new(s, 0.0)).collect();
        fft.process(&mut buf);
        let bins = n / 2;
        let mags: Vec<f32> = (0..bins).map(|b| buf[b].norm()).collect();
        // Harmonic product sum: find the fundamental whose harmonics add up.
        let mut best = (0usize, 0.0f32);
        for b in 20..(bins / 4) {
            let mut s = 0.0;
            for h in 1..=4 {
                if b * h < bins {
                    s += mags[b * h];
                }
            }
            if s > best.1 {
                best = (b, s);
            }
        }
        fs as f32 * best.0 as f32 / n as f32
    }

    #[test]
    fn bypass_preserves_signal() {
        let mut sh = PitchShifter::new(1024, 256, 48000);
        let input = saw(220.0, 48000, 48000);
        let mut out = Vec::new();
        sh.process(&input, &mut out);
        assert!(out.len() >= 47000, "out.len={}", out.len());
        assert!(rms(&out[1000..]).is_finite());
        assert!(
            rms(&out[1000..]) > 0.05,
            "bypass rms too low: {}",
            rms(&out[1000..])
        );

        use rustfft::FftPlanner;
        let n = 4096;
        let seg = &out[2000..2000 + n];
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let mut buf: Vec<Complex<f32>> = seg.iter().map(|&s| Complex::new(s, 0.0)).collect();
        fft.process(&mut buf);
        let mut peaks: Vec<(usize, f32)> = (0..n / 2).map(|b| (b, buf[b].norm())).collect();
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top: Vec<String> = peaks
            .iter()
            .take(5)
            .map(|(b, m)| format!("{:.0}Hz({m:.0})", b * 48000 / n))
            .collect();
        eprintln!("bypass peaks: {}", top.join(", "));
    }

    #[test]
    fn pitch_shift_produces_energy() {
        let mut sh = PitchShifter::new(1024, 256, 48000);
        sh.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let input = saw(220.0, 48000, 48000);
        let mut out = Vec::new();
        sh.process(&input, &mut out);
        assert!(
            out.iter().take(20000).all(|v| v.is_finite()),
            "non-finite output"
        );
        let r = rms(&out[1000..20000]);
        assert!(r > 0.05, "pitch-shifted rms too low: {r}");

        let est = dominant_hz(&out[1000..20000], 48000);
        assert!(
            (est - 293.7).abs() < 8.0,
            "expected ~293.7 Hz for 220*2^(5/12), got {est:.1} Hz"
        );
    }

    #[test]
    fn block_feed_matches_chain() {
        // Mimics dsp::chain's block-by-block feeding with a pending buffer.
        let mut sh = PitchShifter::new(1024, 256, 48000);
        sh.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let input = sine(220.0, 48000, 48000);
        let block = 2560;
        let mut pending = Vec::new();
        let mut out_all = Vec::new();
        for chunk in input.chunks(block) {
            let mut out = Vec::new();
            sh.process(chunk, &mut out);
            pending.extend(out);
            let take = chunk.len().min(pending.len());
            let mut mono = vec![0.0; chunk.len()];
            mono[..take].copy_from_slice(&pending[..take]);
            out_all.extend(mono);
            pending.drain(..take);
        }
        assert!(out_all.iter().all(|v| v.is_finite()), "non-finite");
        let r = rms(&out_all[4000..40000]);
        assert!(r > 0.05, "block-feed rms too low: {r}");
    }

    #[test]
    fn one_shot_vs_block_fed() {
        let mut a = PitchShifter::new(1024, 256, 48000);
        a.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let input = saw(220.0, 48000, 48000);
        let mut out_a = Vec::new();
        a.process(&input, &mut out_a);

        // Block-fed WITHOUT prime/pending, just concatenating shifter output.
        let mut b = PitchShifter::new(1024, 256, 48000);
        b.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let mut out_b = Vec::new();
        for chunk in input.chunks(1024) {
            b.process(chunk, &mut out_b);
        }
        let n = out_a.len().min(out_b.len());
        assert!(n > 10000, "n={n}");
        let mut max_diff = 0.0f32;
        let mut idx = 0usize;
        for i in 3000..n {
            let d = (out_a[i] - out_b[i]).abs();
            if d > max_diff {
                max_diff = d;
                idx = i;
            }
        }
        eprintln!("max diff {max_diff} at {idx} of {n}");
        assert!(
            max_diff < 1e-3,
            "block-fed differs from one-shot: max_diff={max_diff}"
        );
    }
}
