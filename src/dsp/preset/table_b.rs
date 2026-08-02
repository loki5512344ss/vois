//! Built-in preset table (part of the list).

use super::{EffectSpec, Preset};
use crate::dsp::fx::CarrierKind;

pub(crate) fn table_b() -> Vec<Preset> {
    vec![
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
            gain: 1.2,
            gate_db: -38.0,
        },
        Preset {
            name: "Ghost".into(),
            pitch_semitones: -3.0,
            formant_ratio: 1.05,
            effects: vec![
                EffectSpec::Reverb {
                    room: 0.85,
                    damp: 0.3,
                    wet: 0.6,
                },
                EffectSpec::Chorus {
                    depth_ms: 5.0,
                    rate_hz: 0.5,
                    wet: 0.3,
                },
                EffectSpec::Tremolo {
                    depth: 0.2,
                    rate_hz: 1.5,
                },
            ],
            gain: 0.80,
            gate_db: -40.0,
        },
        Preset {
            name: "Cyborg".into(),
            pitch_semitones: 2.0,
            formant_ratio: 1.0,
            effects: vec![
                EffectSpec::Vocoder {
                    bands: 16,
                    carrier: CarrierKind::Saw,
                    carrier_hz: 220.0,
                    wet: 0.9,
                },
                EffectSpec::Overdrive {
                    drive: 1.5,
                    output: 1.0,
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
            name: "Chipmunk".into(),
            pitch_semitones: 7.0,
            formant_ratio: 1.4,
            effects: vec![EffectSpec::Chorus {
                depth_ms: 3.0,
                rate_hz: 0.8,
                wet: 0.2,
            }],
            gain: 1.0,
            gate_db: -45.0,
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
            gain: 1.0,
            gate_db: -45.0,
        },
        Preset {
            name: "Vader".into(),
            pitch_semitones: -10.0,
            formant_ratio: 0.6,
            effects: vec![
                EffectSpec::Overdrive {
                    drive: 2.0,
                    output: 0.8,
                },
                EffectSpec::Reverb {
                    room: 0.6,
                    damp: 0.4,
                    wet: 0.3,
                },
                EffectSpec::Tremolo {
                    depth: 0.1,
                    rate_hz: 2.0,
                },
            ],
            gain: 1.4,
            gate_db: -38.0,
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
            gain: 1.0,
            gate_db: -42.0,
        },
        Preset {
            name: "Walkie-Talkie".into(),
            pitch_semitones: 0.0,
            formant_ratio: 1.05,
            effects: vec![
                EffectSpec::Bandpass {
                    low_hz: 500.0,
                    high_hz: 2400.0,
                    mix: 1.0,
                },
                EffectSpec::Noise { gain: 0.05 },
                EffectSpec::Compressor {
                    threshold_db: -20.0,
                    ratio: 4.0,
                    makeup_db: 4.0,
                },
                EffectSpec::Bitcrush {
                    bits: 8,
                    sample_rate_div: 2,
                    max_gain: 4.0,
                    target: 0.6,
                },
            ],
            gain: 1.00,
            gate_db: -40.0,
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
                EffectSpec::Compressor {
                    threshold_db: -18.0,
                    ratio: 4.0,
                    makeup_db: 4.0,
                },
                EffectSpec::Reverb {
                    room: 0.3,
                    damp: 0.5,
                    wet: 0.2,
                },
            ],
            gain: 1.0,
            gate_db: -38.0,
        },
    ]
}
