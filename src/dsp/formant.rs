//! Formant shifting helpers.
//!
//! Formant shifting (changing the vocal-tract resonances without changing
//! pitch) is performed by the phase vocoder in [`crate::dsp::pitch`] via its
//! `formant_ratio` parameter. This module holds the tiny helper used to convert
//! a manual formant adjustment in semitones into the ratio the vocoder expects.

/// Convert a formant adjustment in semitones into a ratio factor
/// (1.0 = unchanged, >1 = higher/brighter).
pub fn ratio_from_semitones(semitones: f32) -> f32 {
    2f32.powf(semitones / 12.0)
}
