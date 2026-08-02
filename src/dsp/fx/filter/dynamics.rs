//! Dynamics: compressor and noise gate.

use super::super::Effect;

pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    makeup_db: f32,
    env: f32,
    gain: f32,
    fs: f32,
}

impl Compressor {
    pub fn new(sample_rate: u32, threshold_db: f32, ratio: f32, makeup_db: f32) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            makeup_db,
            env: 0.0,
            gain: 1.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for Compressor {
    fn process(&mut self, mono: &mut [f32]) {
        let threshold = 10f32.powf(self.threshold_db / 20.0);
        let makeup = 10f32.powf(self.makeup_db / 20.0);
        let release = 1.0 / (self.fs * 0.1).max(1.0);
        let attack = 1.0 / (self.fs * 0.002).max(1.0);
        let inv_ratio = 1.0 - 1.0 / self.ratio;

        for s in mono.iter_mut() {
            let peak = s.abs();
            self.env *= 1.0 - release;
            if peak > self.env {
                self.env += (peak - self.env) * attack;
            }

            let over_db = 20.0 * (self.env / threshold.max(1e-9)).log10();
            let target_gain = if over_db > 0.0 {
                let g_db = -over_db * inv_ratio;
                10f32.powf(g_db / 20.0)
            } else {
                1.0
            };
            self.gain += (target_gain - self.gain) * attack;
            *s *= self.gain * makeup;
        }
    }
}

/// Noise gate: suppresses quiet segments with hysteresis and smoothing.
pub struct NoiseGate {
    pub threshold_db: f32,
    level: f32,
    gain: f32,
    open: bool,
}

impl NoiseGate {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            threshold_db,
            level: 0.0,
            gain: 0.0,
            open: false,
        }
    }

    pub fn set_threshold_db(&mut self, db: f32) {
        self.threshold_db = db;
    }
}

impl Effect for NoiseGate {
    fn process(&mut self, mono: &mut [f32]) {
        if mono.is_empty() {
            return;
        }
        let mut sum = 0.0;
        for &s in mono.iter() {
            sum += s * s;
        }
        let rms = (sum / mono.len() as f32).sqrt();

        if rms > self.level {
            self.level += (rms - self.level) * 0.5;
        } else {
            self.level += (rms - self.level) * 0.02;
        }

        let threshold = 10f32.powf(self.threshold_db / 20.0);
        if self.level > threshold {
            self.open = true;
        } else if self.level < threshold * 0.5 {
            self.open = false;
        }

        let target = if self.open { 1.0 } else { 0.0 };
        for s in mono.iter_mut() {
            self.gain += (target - self.gain) * 0.15;
            *s *= self.gain;
        }
    }
}
