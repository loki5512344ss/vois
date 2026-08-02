//! Generator/modulation effects: white noise, tremolo, ring modulation.

use super::Effect;
use std::f32::consts::PI;

pub struct Noise {
    gain: f32,
    state: u32,
}

impl Noise {
    pub fn new(gain: f32) -> Self {
        Self {
            gain,
            state: 0x9E3779B9,
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

impl Effect for Noise {
    fn process(&mut self, mono: &mut [f32]) {
        for s in mono.iter_mut() {
            *s += self.next() * self.gain;
        }
    }
}

pub struct Tremolo {
    depth: f32,
    rate: f32,
    phase: f32,
    fs: f32,
}

impl Tremolo {
    pub fn new(sample_rate: u32, depth: f32, rate_hz: f32) -> Self {
        Self {
            depth: depth.clamp(0.0, 1.0),
            rate: rate_hz,
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for Tremolo {
    fn process(&mut self, mono: &mut [f32]) {
        for s in mono.iter_mut() {
            let lfo = (2.0 * PI * self.phase).sin();
            let amp = 1.0 - self.depth * (0.5 - 0.5 * lfo);
            *s *= amp;
            self.phase += self.rate / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

pub struct RingMod {
    freq: f32,
    depth: f32,
    phase: f32,
    fs: f32,
}

impl RingMod {
    pub fn new(sample_rate: u32, freq_hz: f32, depth: f32) -> Self {
        Self {
            freq: freq_hz,
            depth: depth.clamp(0.0, 1.0),
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for RingMod {
    fn process(&mut self, mono: &mut [f32]) {
        let depth = self.depth;
        for s in mono.iter_mut() {
            let m = 1.0 - depth + depth * (2.0 * PI * self.phase).sin();
            *s *= m;
            self.phase += self.freq / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}
