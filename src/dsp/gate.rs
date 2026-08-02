//! Noise gate: suppresses quiet segments below the threshold with a short
//! attack / longer release and a 6 dB hysteresis to avoid chatter.

/// Convert dB to linear amplitude.
pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

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

    pub fn process(&mut self, block: &mut [f32]) {
        if block.is_empty() {
            return;
        }

        let mut sum = 0.0;
        for &s in block.iter() {
            sum += s * s;
        }
        let rms = (sum / block.len() as f32).sqrt();

        // Envelope follower: fast attack, slow release.
        if rms > self.level {
            self.level += (rms - self.level) * 0.5;
        } else {
            self.level += (rms - self.level) * 0.02;
        }

        let threshold = db_to_lin(self.threshold_db);

        // Hysteresis: open above threshold, close 6 dB below it.
        if self.level > threshold {
            self.open = true;
        } else if self.level < threshold * 0.5 {
            self.open = false;
        }

        let target = if self.open { 1.0 } else { 0.0 };
        for s in block.iter_mut() {
            // Per-sample smoothing so the gate never clicks.
            self.gain += (target - self.gain) * 0.15;
            *s *= self.gain;
        }
    }
}
