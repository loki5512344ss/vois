//! Preset voice settings: pitch, formant and an effect chain.
//! The list is built at startup from built-in presets plus any custom presets
//! defined in `~/.config/vois/config.toml`.

use crate::dsp::effects::{self, CarrierKind, Effect};
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
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EffectSpec {
    Distortion {
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
        carrier: CarrierKind,
        wet: f32,
    },
}

/// User-defined preset from the config file.
#[derive(Deserialize)]
pub struct CustomPresetCfg {
    pub pitch: Option<f32>,
    pub formant: Option<f32>,
    pub effects: Option<Vec<EffectSpec>>,
}

static PRESETS: OnceLock<Vec<Preset>> = OnceLock::new();

fn builtin_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Clean".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![],
            gain: 1.10,
        },
        Preset {
            name: "Girl / Anime".into(),
            pitch_semitones: 5.0,
            formant_ratio: 1.25,
            effects: vec![],
            gain: 1.50,
        },
        Preset {
            name: "Boy".into(),
            pitch_semitones: 3.0,
            formant_ratio: 1.12,
            effects: vec![],
            gain: 1.00,
        },
        Preset {
            name: "Manly / Deep".into(),
            pitch_semitones: -4.0,
            formant_ratio: 0.78,
            effects: vec![EffectSpec::Compressor {
                threshold_db: -20.0,
                ratio: 3.0,
                makeup_db: 2.0,
            }],
            gain: 1.50,
        },
        Preset {
            name: "Demon".into(),
            pitch_semitones: -9.0,
            formant_ratio: 0.85,
            effects: vec![
                EffectSpec::Distortion {
                    drive: 6.0,
                    output: 0.7,
                },
                EffectSpec::Reverb {
                    room: 0.6,
                    damp: 0.4,
                    wet: 0.4,
                },
            ],
            gain: 1.00,
        },
        Preset {
            name: "Robot".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Vocoder {
                    bands: 16,
                    carrier: CarrierKind::Noise,
                    wet: 1.0,
                },
                EffectSpec::Bitcrush {
                    bits: 8,
                    sample_rate_div: 2,
                    max_gain: 8.0,
                    target: 0.6,
                },
            ],
            gain: 6.00,
        },
        Preset {
            name: "8-bit".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![EffectSpec::Bitcrush {
                bits: 5,
                sample_rate_div: 4,
                max_gain: 32.0,
                target: 0.6,
            }],
            gain: 1.00,
        },
        Preset {
            name: "Alien".into(),
            pitch_semitones: -2.0,
            formant_ratio: 1.15,
            effects: vec![
                EffectSpec::RingMod {
                    freq_hz: 55.0,
                    depth: 0.5,
                },
                EffectSpec::Chorus {
                    depth_ms: 6.0,
                    rate_hz: 0.5,
                    wet: 0.4,
                },
            ],
            gain: 5.50,
        },
        Preset {
            name: "Radio".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Bandpass {
                    low_hz: 300.0,
                    high_hz: 3000.0,
                    mix: 0.9,
                },
                EffectSpec::Noise { gain: 0.04 },
                EffectSpec::Compressor {
                    threshold_db: -24.0,
                    ratio: 4.0,
                    makeup_db: 6.0,
                },
            ],
            gain: 1.00,
        },
        Preset {
            name: "Megaphone".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Distortion {
                    drive: 2.5,
                    output: 0.9,
                },
                EffectSpec::Bandpass {
                    low_hz: 800.0,
                    high_hz: 2600.0,
                    mix: 1.0,
                },
                EffectSpec::Compressor {
                    threshold_db: -18.0,
                    ratio: 5.0,
                    makeup_db: 4.0,
                },
            ],
            gain: 1.60,
        },
        Preset {
            name: "Ghost".into(),
            pitch_semitones: -3.0,
            formant_ratio: 1.05,
            effects: vec![
                EffectSpec::Reverb {
                    room: 0.95,
                    damp: 0.2,
                    wet: 0.8,
                },
                EffectSpec::Chorus {
                    depth_ms: 8.0,
                    rate_hz: 0.4,
                    wet: 0.5,
                },
            ],
            gain: 1.00,
        },
        Preset {
            name: "Cyborg".into(),
            pitch_semitones: 2.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Vocoder {
                    bands: 16,
                    carrier: CarrierKind::Saw,
                    wet: 0.9,
                },
                EffectSpec::Bitcrush {
                    bits: 8,
                    sample_rate_div: 2,
                    max_gain: 8.0,
                    target: 0.6,
                },
            ],
            gain: 1.20,
        },
        Preset {
            name: "Chipmunk".into(),
            pitch_semitones: 7.0,
            formant_ratio: 1.4,
            effects: vec![EffectSpec::Bitcrush {
                bits: 8,
                sample_rate_div: 2,
                max_gain: 8.0,
                target: 0.5,
            }],
            gain: 1.00,
        },
        Preset {
            name: "Helium".into(),
            pitch_semitones: 10.0,
            formant_ratio: 1.5,
            effects: vec![EffectSpec::Chorus {
                depth_ms: 5.0,
                rate_hz: 0.6,
                wet: 0.3,
            }],
            gain: 1.20,
        },
        Preset {
            name: "Vader".into(),
            pitch_semitones: -10.0,
            formant_ratio: 0.6,
            effects: vec![EffectSpec::Reverb {
                room: 0.7,
                damp: 0.3,
                wet: 0.35,
            }],
            gain: 2.80,
        },
        Preset {
            name: "Telephone".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Bandpass {
                    low_hz: 300.0,
                    high_hz: 3400.0,
                    mix: 1.0,
                },
                EffectSpec::Noise { gain: 0.02 },
                EffectSpec::Compressor {
                    threshold_db: -22.0,
                    ratio: 4.0,
                    makeup_db: 8.0,
                },
            ],
            gain: 1.00,
        },
        Preset {
            name: "Growl".into(),
            pitch_semitones: -5.0,
            formant_ratio: 0.7,
            effects: vec![
                EffectSpec::Distortion {
                    drive: 4.0,
                    output: 0.6,
                },
                EffectSpec::Reverb {
                    room: 0.5,
                    damp: 0.4,
                    wet: 0.3,
                },
            ],
            gain: 1.00,
        },
    ]
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

pub fn build_effects(specs: &[EffectSpec], sample_rate: u32) -> Vec<Box<dyn Effect>> {
    specs.iter().map(|s| build_effect(s, sample_rate)).collect()
}

fn build_effect(spec: &EffectSpec, sample_rate: u32) -> Box<dyn Effect> {
    match spec {
        EffectSpec::Distortion { drive, output } => {
            Box::new(effects::Distortion::new(*drive, *output))
        }
        EffectSpec::Bitcrush {
            bits,
            sample_rate_div,
            max_gain,
            target,
        } => Box::new(effects::Bitcrush::new(
            *bits,
            *sample_rate_div,
            *max_gain,
            *target,
        )),
        EffectSpec::Reverb { room, damp, wet } => {
            Box::new(effects::Reverb::new(sample_rate, *room, *damp, *wet))
        }
        EffectSpec::Chorus {
            depth_ms,
            rate_hz,
            wet,
        } => Box::new(effects::Chorus::new(sample_rate, *depth_ms, *rate_hz, *wet)),
        EffectSpec::RingMod { freq_hz, depth } => {
            Box::new(effects::RingMod::new(sample_rate, *freq_hz, *depth))
        }
        EffectSpec::Bandpass {
            low_hz,
            high_hz,
            mix,
        } => Box::new(effects::Bandpass::new(sample_rate, *low_hz, *high_hz, *mix)),
        EffectSpec::Noise { gain } => Box::new(effects::Noise::new(*gain)),
        EffectSpec::Compressor {
            threshold_db,
            ratio,
            makeup_db,
        } => Box::new(effects::Compressor::new(
            sample_rate,
            *threshold_db,
            *ratio,
            *makeup_db,
        )),
        EffectSpec::Vocoder {
            bands,
            carrier,
            wet,
        } => Box::new(effects::ChannelVocoder::new(
            sample_rate,
            *bands,
            *carrier,
            *wet,
        )),
    }
}
