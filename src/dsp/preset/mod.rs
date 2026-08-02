//! Preset voice settings: pitch, formant, effect chain and gate.
//! Built at startup from built-ins plus custom presets from the config file.

pub mod build;
pub mod table_a;
pub mod table_b;

pub use build::build_effects;

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub struct Preset {
    pub name: String,
    /// Pitch shift in semitones (positive = higher).
    pub pitch_semitones: f32,
    /// Formant ratio (1.0 = unchanged, >1 = higher/brighter).
    pub formant_ratio: f32,
    /// Effects applied in order after pitch/formant.
    pub effects: Vec<EffectSpec>,
    /// Output makeup gain so every preset sounds equally loud.
    pub gain: f32,
    /// Noise gate threshold for this preset (-100 = off).
    pub gate_db: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EffectSpec {
    Distortion {
        drive: f32,
        output: f32,
    },
    Overdrive {
        drive: f32,
        output: f32,
    },
    Bitcrush {
        bits: u8,
        sample_rate_div: u32,
        max_gain: f32,
        target: f32,
    },
    Reverb {
        room: f32,
        damp: f32,
        wet: f32,
    },
    Chorus {
        depth_ms: f32,
        rate_hz: f32,
        wet: f32,
    },
    Flanger {
        depth_ms: f32,
        rate_hz: f32,
        feedback: f32,
        wet: f32,
    },
    Tremolo {
        depth: f32,
        rate_hz: f32,
    },
    RingMod {
        freq_hz: f32,
        depth: f32,
    },
    Bandpass {
        low_hz: f32,
        high_hz: f32,
        mix: f32,
    },
    Noise {
        gain: f32,
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        makeup_db: f32,
    },
    Vocoder {
        bands: usize,
        carrier: crate::dsp::fx::CarrierKind,
        carrier_hz: f32,
        wet: f32,
    },
}

/// User-defined preset from the config file.
#[derive(Deserialize)]
pub struct CustomPresetCfg {
    pub pitch: Option<f32>,
    pub formant: Option<f32>,
    pub effects: Option<Vec<EffectSpec>>,
    pub gate_db: Option<f32>,
}

static PRESETS: OnceLock<Vec<Preset>> = OnceLock::new();

fn builtin_presets() -> Vec<Preset> {
    let mut all = table_a::table_a();
    all.extend(table_b::table_b());
    all
}

/// Register custom presets (from the config file). Call once at startup.
pub fn init_custom(customs: Vec<Preset>) {
    let _ = PRESETS.get_or_init(|| {
        let mut all = builtin_presets();
        all.extend(customs);
        all
    });
}

/// The full preset list (built-ins + customs).
pub fn presets() -> &'static [Preset] {
    init_custom(Vec::new());
    PRESETS.get_or_init(builtin_presets)
}

impl Preset {
    pub fn find(name: &str) -> Option<usize> {
        presets().iter().position(|p| {
            p.name.eq_ignore_ascii_case(name)
                || p.name
                    .split('/')
                    .next()
                    .map(|s| s.trim().eq_ignore_ascii_case(name))
                    .unwrap_or(false)
        })
    }
}
