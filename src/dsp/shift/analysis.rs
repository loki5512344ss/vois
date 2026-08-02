//! Frame analysis: fundamental detection and cepstral envelope.

use super::shifter::PitchShifter;
use rustfft::num_complex::Complex;

impl PitchShifter {
    /// Fundamental frequency via harmonic product sum. Sets `last_f0` (Hz),
    /// or 0.0 when the frame is unvoiced/weak.
    pub(crate) fn detect_f0(&mut self) {
        let n2 = self.n / 2;
        let bin_hz = self.fs as f32 / self.n as f32;
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
        let peak = self.mag[..n2].iter().cloned().fold(0.0f32, f32::max);
        self.last_f0 = if best_bin > 0 && self.mag[best_bin] > peak * 0.05 && best_score > 0.0 {
            best_bin as f32 * bin_hz
        } else {
            0.0
        };
    }

    /// Spectral envelope via homomorphic (cepstral) filtering.
    pub(crate) fn compute_envelope(&mut self) {
        let n2 = self.n / 2;

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
