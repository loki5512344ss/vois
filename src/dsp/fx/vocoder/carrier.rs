//! Vocoder carrier kinds and generators.

#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum CarrierKind {
    Noise,
    Saw,
    Input,
}
