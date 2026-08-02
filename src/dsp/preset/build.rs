//! Effect construction: EffectSpec -> Box<dyn Effect>.

use super::EffectSpec;
use crate::dsp::fx::{self, Effect};

pub fn build_effects(specs: &[EffectSpec], sample_rate: u32) -> Vec<Box<dyn Effect>> {
    specs.iter().map(|s| build_effect(s, sample_rate)).collect()
}

fn build_effect(spec: &EffectSpec, sample_rate: u32) -> Box<dyn Effect> {
    match spec {
        EffectSpec::Distortion { drive, output } => Box::new(fx::Distortion::new(*drive, *output)),
        EffectSpec::Overdrive { drive, output } => Box::new(fx::Overdrive::new(*drive, *output)),
        EffectSpec::Bitcrush {
            bits,
            sample_rate_div,
            max_gain,
            target,
        } => Box::new(fx::Bitcrush::new(
            *bits,
            *sample_rate_div,
            *max_gain,
            *target,
        )),
        EffectSpec::Reverb { room, damp, wet } => {
            Box::new(fx::Reverb::new(sample_rate, *room, *damp, *wet))
        }
        EffectSpec::Chorus {
            depth_ms,
            rate_hz,
            wet,
        } => Box::new(fx::Chorus::new(sample_rate, *depth_ms, *rate_hz, *wet)),
        EffectSpec::Flanger {
            depth_ms,
            rate_hz,
            feedback,
            wet,
        } => Box::new(fx::Flanger::new(
            sample_rate,
            *depth_ms,
            *rate_hz,
            *feedback,
            *wet,
        )),
        EffectSpec::Tremolo { depth, rate_hz } => {
            Box::new(fx::Tremolo::new(sample_rate, *depth, *rate_hz))
        }
        EffectSpec::RingMod { freq_hz, depth } => {
            Box::new(fx::RingMod::new(sample_rate, *freq_hz, *depth))
        }
        EffectSpec::Bandpass {
            low_hz,
            high_hz,
            mix,
        } => Box::new(fx::Bandpass::new(sample_rate, *low_hz, *high_hz, *mix)),
        EffectSpec::Noise { gain } => Box::new(fx::Noise::new(*gain)),
        EffectSpec::Compressor {
            threshold_db,
            ratio,
            makeup_db,
        } => Box::new(fx::Compressor::new(
            sample_rate,
            *threshold_db,
            *ratio,
            *makeup_db,
        )),
        EffectSpec::Vocoder {
            bands,
            carrier,
            carrier_hz,
            wet,
        } => Box::new(fx::ChannelVocoder::new(
            sample_rate,
            *bands,
            *carrier,
            *carrier_hz,
            *wet,
        )),
    }
}
