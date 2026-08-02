//! Biquad filters (RBJ cookbook).

use super::super::Effect;
use std::f32::consts::PI;

pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Constant-skirt bandpass.
    pub fn bandpass(sample_rate: u32, f0: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * f0 / sample_rate as f32;
        let alpha = w0.sin() / (2.0 * q);
        Self::normalized(
            alpha,
            0.0,
            -alpha,
            -2.0 * w0.cos(),
            1.0 - alpha,
            1.0 + alpha,
        )
    }

    fn normalized(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, a0: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct Bandpass {
    filter: Biquad,
    mix: f32,
}

impl Bandpass {
    pub fn new(sample_rate: u32, low_hz: f32, high_hz: f32, mix: f32) -> Self {
        let f0 = (low_hz * high_hz).sqrt();
        let q = f0 / (high_hz - low_hz).max(1.0);
        Self {
            filter: Biquad::bandpass(sample_rate, f0, q),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

impl Effect for Bandpass {
    fn process(&mut self, mono: &mut [f32]) {
        let mix = self.mix;
        for s in mono.iter_mut() {
            let y = self.filter.tick(*s);
            *s = *s * (1.0 - mix) + y * mix;
        }
    }
}
