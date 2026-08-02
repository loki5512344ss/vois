//! Effects library: small processing units applied to the mono stream after
//! pitch/formant shifting. Each effect mutates the block in place.

mod drive;
mod modgen;

pub mod filter;
pub mod time;
pub mod vocoder;

pub use drive::{Bitcrush, Distortion, Overdrive};
pub use filter::{Bandpass, Compressor};
pub use modgen::{Noise, RingMod, Tremolo};
pub use time::{Chorus, Flanger, Reverb};
pub use vocoder::{CarrierKind, ChannelVocoder};

pub trait Effect: Send {
    fn process(&mut self, mono: &mut [f32]);
}
