//! Built-in preset table (part of the list).

use super::{EffectSpec, Preset};
use crate::dsp::fx::CarrierKind;

pub(crate) fn table_a() -> Vec<Preset> {
    vec![
        Preset {
            name: "Clean".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![],
            gain: 1.1,
            gate_db: -100.0,
        },
        Preset {
            name: "Girl / Anime".into(),
            pitch_semitones: 5.0,
            formant_ratio: 1.25,
            effects: vec![
                EffectSpec::Chorus {
                    depth_ms: 4.0,
                    rate_hz: 0.6,
                    wet: 0.25,
                },
                EffectSpec::Overdrive {
                    drive: 1.4,
                    output: 1.0,
                },
            ],
            gain: 1.3,
            gate_db: -45.0,
        },
        Preset {
            name: "Boy".into(),
            pitch_semitones: 2.0,
            formant_ratio: 1.08,
            effects: vec![EffectSpec::Compressor {
                threshold_db: -20.0,
                ratio: 3.0,
                makeup_db: 4.0,
            }],
            gain: 1.1,
            gate_db: -45.0,
        },
        Preset {
            name: "Manly / Deep".into(),
            pitch_semitones: -4.0,
            formant_ratio: 0.78,
            effects: vec![
                EffectSpec::Compressor {
                    threshold_db: -20.0,
                    ratio: 3.0,
                    makeup_db: 2.0,
                },
                EffectSpec::Reverb {
                    room: 0.25,
                    damp: 0.5,
                    wet: 0.15,
                },
            ],
            gain: 1.3,
            gate_db: -40.0,
        },
        Preset {
            name: "Demon".into(),
            pitch_semitones: -9.0,
            formant_ratio: 0.85,
            effects: vec![
                EffectSpec::Overdrive {
                    drive: 2.5,
                    output: 0.8,
                },
                EffectSpec::Reverb {
                    room: 0.5,
                    damp: 0.45,
                    wet: 0.3,
                },
                EffectSpec::Tremolo {
                    depth: 0.15,
                    rate_hz: 4.0,
                },
            ],
            gain: 1.0,
            gate_db: -38.0,
        },
        Preset {
            name: "Robot".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Vocoder {
                    bands: 16,
                    carrier: CarrierKind::Noise,
                    carrier_hz: 110.0,
                    wet: 1.0,
                },
                EffectSpec::Bitcrush {
                    bits: 8,
                    sample_rate_div: 2,
                    max_gain: 4.0,
                    target: 0.6,
                },
            ],
            gain: 1.8,
            gate_db: -40.0,
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
            gain: 1.0,
            gate_db: -45.0,
        },
        Preset {
            name: "Alien".into(),
            pitch_semitones: -2.0,
            formant_ratio: 1.15,
            effects: vec![
                EffectSpec::RingMod {
                    freq_hz: 140.0,
                    depth: 0.5,
                },
                EffectSpec::Flanger {
                    depth_ms: 2.5,
                    rate_hz: 0.4,
                    feedback: 0.5,
                    wet: 0.5,
                },
            ],
            gain: 2.0,
            gate_db: -40.0,
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
            gain: 1.0,
            gate_db: -40.0,
        },
    ]
}
