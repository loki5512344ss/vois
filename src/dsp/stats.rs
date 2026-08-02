//! Runtime statistics produced by the DSP worker and shown in the TUI.

use std::time::Instant;

pub struct Stats {
    /// Smoothed input level (RMS, linear).
    pub input_rms: f32,
    /// Smoothed output level (RMS, linear).
    pub output_rms: f32,
    pub peak_in: f32,
    pub peak_out: f32,
    pub blocks: u64,
    pub samples: u64,
    /// Smoothed DSP load in percent of one core.
    pub cpu_pct: f32,
    /// Estimated DSP latency in milliseconds.
    pub dsp_latency_ms: f32,
    pub started: Instant,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            input_rms: 0.0,
            output_rms: 0.0,
            peak_in: 0.0,
            peak_out: 0.0,
            blocks: 0,
            samples: 0,
            cpu_pct: 0.0,
            dsp_latency_ms: 0.0,
            started: Instant::now(),
        }
    }
}
