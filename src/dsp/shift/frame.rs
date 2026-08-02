//! Per-frame FFT processing: pitch/formant resampling with phase propagation.

use super::shifter::PitchShifter;
use super::{interpolate, princarg};
use rustfft::num_complex::Complex;

impl PitchShifter {
    pub(crate) fn process_frame(&mut self) {
        let n2 = self.n / 2;

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

        self.compute_envelope();
        self.detect_f0();

        let pr = self.pitch_ratio;
        let fr = self.formant_ratio;
        let p_active = (pr - 1.0).abs() > 1e-4;
        let f_active = (fr - 1.0).abs() > 1e-4;

        if p_active || f_active {
            // Phase propagation (Laroche & Dolson): track the instantaneous
            // frequency of each input bin so reconstructed components stay
            // coherent across frames.
            let hop = self.hop as f32;
            let nf = self.n as f32;
            let mut new_mag = vec![0.0; n2 + 1];
            let mut new_phase = vec![0.0; n2 + 1];
            for (b, nm) in new_mag.iter_mut().enumerate() {
                let bf = b as f32;

                // Source input bin for output bin b under the pitch shift.
                let src = if p_active { bf / pr } else { bf };
                let base = interpolate(&self.mag, src);

                // Formants: move the envelope shape by `fr` independently.
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
}
