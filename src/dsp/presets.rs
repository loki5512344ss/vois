//! Preset voice settings: pitch, formant and an effect chain.

use crate::dsp::effects::{self, CarrierKind, Effect};

pub struct Preset {
    pub name: &'static str,
    /// Pitch shift in semitones (positive = higher).
    pub pitch_semitones: f32,
    /// Formant ratio (1.0 = unchanged, >1 = higher/brighter).
    pub formant_ratio: f32,
    /// Effects applied in order after pitch/formant.
    pub effects: &'static [EffectSpec],
}

#[derive(Clone, Copy, Debug)]
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

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Clean",
        pitch_semitones: 0.0,
        formant_ratio: 1.0,
        effects: &[],
    },
    Preset {
        name: "Girl / Anime",
        pitch_semitones: 5.0,
        formant_ratio: 1.25,
        effects: &[],
    },
    Preset {
        name: "Boy",
        pitch_semitones: 3.0,
        formant_ratio: 1.12,
        effects: &[],
    },
    Preset {
        name: "Manly / Deep",
        pitch_semitones: -4.0,
        formant_ratio: 0.78,
        effects: &[EffectSpec::Compressor {
            threshold_db: -20.0,
            ratio: 3.0,
            makeup_db: 2.0,
        }],
    },
    Preset {
        name: "Demon",
        pitch_semitones: -9.0,
        formant_ratio: 0.85,
        effects: &[
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
    },
    Preset {
        name: "Robot",
        pitch_semitones: 0.0,
        formant_ratio: 1.0,
        effects: &[
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
    },
    Preset {
        name: "8-bit",
        pitch_semitones: 0.0,
        formant_ratio: 1.0,
        effects: &[EffectSpec::Bitcrush {
            bits: 5,
            sample_rate_div: 4,
            max_gain: 32.0,
            target: 0.6,
        }],
    },
    Preset {
        name: "Alien",
        pitch_semitones: -2.0,
        formant_ratio: 1.15,
        effects: &[
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
    },
    Preset {
        name: "Radio",
        pitch_semitones: 0.0,
        formant_ratio: 1.0,
        effects: &[
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
    },
    Preset {
        name: "Megaphone",
        pitch_semitones: 0.0,
        formant_ratio: 1.0,
        effects: &[
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
    },
    Preset {
        name: "Ghost",
        pitch_semitones: -3.0,
        formant_ratio: 1.05,
        effects: &[
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
    },
    Preset {
        name: "Cyborg",
        pitch_semitones: 2.0,
        formant_ratio: 1.0,
        effects: &[
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
    },
];

impl Preset {
    pub fn find(name: &str) -> Option<usize> {
        PRESETS.iter().position(|p| {
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
