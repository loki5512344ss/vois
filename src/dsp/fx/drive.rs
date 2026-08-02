//! Drive effects: waveshapers and bit crusher.

use super::Effect;

pub struct Distortion {
    drive: f32,
    output: f32,
}

impl Distortion {
    pub fn new(drive: f32, output: f32) -> Self {
        Self { drive, output }
    }
}

impl Effect for Distortion {
    fn process(&mut self, mono: &mut [f32]) {
        for s in mono.iter_mut() {
            *s = (*s * self.drive).tanh() * self.output;
        }
    }
}

pub struct Overdrive {
    drive: f32,
    output: f32,
}

impl Overdrive {
    pub fn new(drive: f32, output: f32) -> Self {
        Self {
            drive: drive.max(0.01),
            output,
        }
    }
}

impl Effect for Overdrive {
    fn process(&mut self, mono: &mut [f32]) {
        let d = self.drive;
        let o = self.output;
        for s in mono.iter_mut() {
            *s = ((*s * d).atan() / d) * o;
        }
    }
}

pub struct Bitcrush {
    bits: u8,
    sample_rate_div: u32,
    /// Max boost applied to quiet input (auto-level normalization).
    max_gain: f32,
    /// Target peak level after normalization.
    target: f32,
    gain_smooth: f32,
    hold: f32,
    count: u32,
}

impl Bitcrush {
    /// `max_gain`/`target` normalize each block before quantization so the
    /// effect works on quiet mics without clipping loud ones.
    pub fn new(bits: u8, sample_rate_div: u32, max_gain: f32, target: f32) -> Self {
        Self {
            bits: bits.max(2),
            sample_rate_div: sample_rate_div.max(1),
            max_gain: max_gain.max(1.0),
            target: target.clamp(0.1, 1.0),
            gain_smooth: 1.0,
            hold: 0.0,
            count: 0,
        }
    }
}

impl Effect for Bitcrush {
    fn process(&mut self, mono: &mut [f32]) {
        if mono.is_empty() {
            return;
        }
        let levels = (1i32 << (self.bits - 1)) as f32;

        let peak = mono.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let gain = if peak > 1e-6 {
            (self.target / peak).clamp(1.0, self.max_gain)
        } else {
            self.max_gain
        };
        self.gain_smooth += (gain - self.gain_smooth) * 0.3;
        let g = self.gain_smooth;

        for s in mono.iter_mut() {
            self.count = (self.count + 1) % self.sample_rate_div;
            if self.count == 0 {
                self.hold = *s;
            }
            let q = (self.hold * g * levels).round();
            *s = (q / levels).clamp(-1.0, 1.0);
        }
    }
}
