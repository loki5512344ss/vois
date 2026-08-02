//! Real-time pitch/formant shifting via phase vocoder.
//!
//! Resamples the spectral *residual* to change pitch while keeping the
//! envelope (formants), and independently resamples the envelope to change
//! formants. Latency is one FFT frame (~21 ms @ 48 kHz).

pub mod shifter;

mod analysis;
mod frame;

pub use shifter::PitchShifter;

/// Linear interpolation over `arr` at fractional position `pos`.
pub(crate) fn interpolate(arr: &[f32], pos: f32) -> f32 {
    let max = arr.len() as f32 - 1.0;
    let pos = pos.clamp(0.0, max);
    let i = pos.floor() as usize;
    let j = (i + 1).min(arr.len() - 1);
    let t = pos - i as f32;
    arr[i] * (1.0 - t) + arr[j] * t
}

/// Wrap a phase difference into (-PI, PI].
pub(crate) fn princarg(x: f32) -> f32 {
    x - 2.0 * std::f32::consts::PI * (x / (2.0 * std::f32::consts::PI)).round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::Complex;

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

    fn sine(freq: f32, fs: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / fs as f32).sin())
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
    fn pitch_shift_produces_energy() {
        let mut sh = PitchShifter::new(1024, 256, 48000);
        sh.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let input = saw(220.0, 48000, 48000);
        let mut out = Vec::new();
        sh.process(&input, &mut out);
        assert!(out.iter().take(20000).all(|v| v.is_finite()), "non-finite");
        let r = rms(&out[1000..20000]);
        assert!(r > 0.05, "rms too low: {r}");
        let est = dominant_hz(&out[1000..20000], 48000);
        assert!(
            (est - 293.7).abs() < 8.0,
            "expected ~293.7 Hz, got {est:.1}"
        );
    }

    #[test]
    fn block_feed_matches_chain() {
        let mut sh = PitchShifter::new(1024, 256, 48000);
        sh.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let input = sine(220.0, 48000, 48000);
        let mut pending = Vec::new();
        let mut out_all = Vec::new();
        for chunk in input.chunks(2560) {
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

        let mut b = PitchShifter::new(1024, 256, 48000);
        b.pitch_ratio = 2f32.powf(5.0 / 12.0);
        let mut out_b = Vec::new();
        for chunk in input.chunks(1024) {
            b.process(chunk, &mut out_b);
        }
        let n = out_a.len().min(out_b.len());
        assert!(n > 10000, "n={n}");
        let mut max_diff = 0.0f32;
        for i in 3000..n {
            let d = (out_a[i] - out_b[i]).abs();
            if d > max_diff {
                max_diff = d;
            }
        }
        assert!(max_diff < 1e-3, "differs: max_diff={max_diff}");
    }
}
