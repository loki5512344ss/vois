//! Shared runtime state: UI control knobs and DSP statistics.

use std::time::Instant;

/// Processing mode, switchable at runtime (A/B compare).
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// Normal processing through the DSP chain.
    Live,
    /// Raw input straight to output (bypass) for A/B comparison.
    Passthrough,
}

impl Mode {
    pub fn toggle(&mut self) {
        *self = match self {
            Mode::Live => Mode::Passthrough,
            Mode::Passthrough => Mode::Live,
        };
    }
}

/// Live-adjustable state, shared between the UI and the DSP worker.
pub struct Control {
    pub preset_idx: usize,
    /// Manual pitch adjustment added to the preset value (semitones).
    pub pitch_delta: f32,
    /// Manual formant adjustment added to the preset value (semitones).
    pub formant_delta: f32,
    pub gate_threshold_db: f32,
    pub gain: f32,
    pub mute: bool,
    pub record: bool,
    pub record_path: Option<String>,
    pub run_seconds: Option<u32>,
    /// Test tone frequency (Hz) mixed into the input; 0 = off.
    pub tone_hz: f32,
    /// Snap the voice to the nearest semitone (Auto-Tune style).
    pub pitch_correct: bool,
    pub mode: Mode,
    pub quit: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            preset_idx: 0,
            pitch_delta: 0.0,
            formant_delta: 0.0,
            gate_threshold_db: -100.0,
            gain: 1.0,
            mute: false,
            record: false,
            record_path: None,
            run_seconds: None,
            tone_hz: 0.0,
            pitch_correct: false,
            mode: Mode::Live,
            quit: false,
        }
    }
}

/// Runtime statistics produced by the DSP worker and shown in the TUI.
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
