//! Channel vocoder: analysis filterbank + envelope followers + synthesis.

use super::super::Effect;
use super::carrier::CarrierKind;
use crate::dsp::fx::filter::Biquad;

pub struct ChannelVocoder {
    bands: Vec<VocoderBand>,
    carrier: CarrierKind,
    bands_count: usize,
    wet: f32,
    makeup: f32,
    carrier_hz: f32,
    phase: f32,
    noise_state: u32,
    fs: f32,
}

struct VocoderBand {
    analysis: Biquad,
    synthesis: Biquad,
    env: f32,
}

impl ChannelVocoder {
    pub fn new(
        sample_rate: u32,
        bands_count: usize,
        carrier: CarrierKind,
        carrier_hz: f32,
        wet: f32,
    ) -> Self {
        let fs = sample_rate as f32;
        let low: f32 = 150.0;
        let high: f32 = 4500.0;
        let n = bands_count.max(2);
        let mut bands = Vec::with_capacity(n);
        for i in 0..n {
            let f = low * (high / low).powf(i as f32 / (n - 1) as f32);
            let f0: f32 = f;
            let q = 8.0;
            bands.push(VocoderBand {
                analysis: Biquad::bandpass(sample_rate, f0, q),
                synthesis: Biquad::bandpass(sample_rate, f0, q),
                env: 0.0,
            });
        }
        Self {
            bands,
            carrier,
            bands_count: n,
            wet: wet.clamp(0.0, 1.0),
            makeup: 5.0,
            carrier_hz: carrier_hz.max(20.0),
            phase: 0.0,
            noise_state: 0x2545F491,
            fs,
        }
    }

    #[inline]
    fn carrier_sample(&mut self, input: f32) -> f32 {
        match self.carrier {
            CarrierKind::Noise => {
                let mut x = self.noise_state;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                self.noise_state = x;
                x as f32 / u32::MAX as f32 * 2.0 - 1.0
            }
            CarrierKind::Saw => {
                let s = self.phase * 2.0 - 1.0;
                self.phase += self.carrier_hz / self.fs;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                s
            }
            CarrierKind::Input => input,
        }
    }
}

impl Effect for ChannelVocoder {
    fn process(&mut self, mono: &mut [f32]) {
        let wet = self.wet;
        let dry = 1.0 - wet;
        let attack = 0.06;
        let release = 0.0015;
        let inv_n = 1.0 / self.bands_count as f32;

        for s in mono.iter_mut() {
            let input = *s;
            let carrier = self.carrier_sample(input);

            let mut acc = 0.0;
            for band in &mut self.bands {
                let a = band.analysis.tick(input);
                let a_env = a.abs();
                if a_env > band.env {
                    band.env += (a_env - band.env) * attack;
                } else {
                    band.env += (a_env - band.env) * release;
                }
                acc += band.synthesis.tick(carrier) * band.env;
            }

            *s = input * dry + acc * wet * inv_n * self.makeup;
        }
    }
}
