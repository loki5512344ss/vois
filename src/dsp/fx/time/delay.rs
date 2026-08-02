//! Modulated delay lines: chorus and flanger.

use super::super::Effect;
use std::f32::consts::PI;

struct DelayLine {
    buf: Vec<f32>,
    idx: usize,
    cap: usize,
}

impl DelayLine {
    fn new(cap: usize) -> Self {
        Self {
            buf: vec![0.0; cap],
            idx: 0,
            cap,
        }
    }

    #[inline]
    fn write(&mut self, value: f32) {
        self.buf[self.idx] = value;
        self.idx = (self.idx + 1) % self.cap;
    }

    #[inline]
    fn read(&self, offset: f32) -> f32 {
        let mut pos = self.idx as f32 - offset;
        if pos < 0.0 {
            pos += self.cap as f32;
        }
        let i = pos.floor() as usize % self.cap;
        let j = (i + 1) % self.cap;
        let t = pos - pos.floor();
        self.buf[i] * (1.0 - t) + self.buf[j] * t
    }
}

pub struct Chorus {
    line: DelayLine,
    depth: f32,
    rate: f32,
    wet: f32,
    phase: f32,
    fs: f32,
}

impl Chorus {
    pub fn new(sample_rate: u32, depth_ms: f32, rate_hz: f32, wet: f32) -> Self {
        let depth = depth_ms / 1000.0 * sample_rate as f32;
        let cap = (sample_rate as f32 * 0.1) as usize + (depth * 4.0) as usize;
        Self {
            line: DelayLine::new(cap),
            depth: depth.max(1.0),
            rate: rate_hz,
            wet: wet.clamp(0.0, 1.0),
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for Chorus {
    fn process(&mut self, mono: &mut [f32]) {
        let depth = self.depth;
        let wet = self.wet;
        let dry = 1.0 - wet;
        for s in mono.iter_mut() {
            self.line.write(*s);
            let lfo = (2.0 * PI * self.phase).sin();
            let v1 = self.line.read(depth * (0.35 + 0.35 * lfo));
            let v2 = self.line.read(depth * (0.35 - 0.35 * lfo));
            *s = *s * dry + (v1 + v2) * 0.5 * wet;
            self.phase += self.rate / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

pub struct Flanger {
    line: DelayLine,
    base: f32,
    depth: f32,
    rate: f32,
    feedback: f32,
    wet: f32,
    phase: f32,
    fs: f32,
}

impl Flanger {
    pub fn new(sample_rate: u32, depth_ms: f32, rate_hz: f32, feedback: f32, wet: f32) -> Self {
        let depth = (depth_ms / 1000.0 * sample_rate as f32).max(1.0);
        let cap = (sample_rate as f32 * 0.1) as usize + (depth * 2.0) as usize + 2;
        Self {
            line: DelayLine::new(cap),
            base: depth * 0.5,
            depth: depth * 0.5,
            rate: rate_hz,
            feedback: feedback.clamp(0.0, 0.9),
            wet: wet.clamp(0.0, 1.0),
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for Flanger {
    fn process(&mut self, mono: &mut [f32]) {
        let wet = self.wet;
        let dry = 1.0 - wet;
        for s in mono.iter_mut() {
            let lfo = (2.0 * PI * self.phase).sin();
            let offset = self.base + self.depth * lfo;
            let delayed = self.line.read(offset);
            let y = *s * dry + delayed * wet;
            self.line.write(*s + delayed * self.feedback);
            *s = y;
            self.phase += self.rate / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}
