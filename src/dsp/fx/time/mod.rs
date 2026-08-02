//! Time-based effects: reverb (Freeverb) and modulated delays (chorus, flanger).

pub mod delay;
pub mod reverb;

pub use delay::{Chorus, Flanger};
pub use reverb::Reverb;
