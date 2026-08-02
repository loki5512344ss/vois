//! The phase-vocoder shifter: streaming interface and buffers.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

pub struct PitchShifter {
    pub(crate) n: usize,
    pub(crate) hop: usize,
    pub(crate) window: Vec<f32>,
    pub(crate) norm: f32,
    pub(crate) fft: Arc<dyn Fft<f32>>,
    pub(crate) ifft: Arc<dyn Fft<f32>>,
    pub(crate) env_ifft: Arc<dyn Fft<f32>>,
    pub(crate) env_fft: Arc<dyn Fft<f32>>,
    pub(crate) in_buf: Vec<f32>,
    pub(crate) out_buf: Vec<f32>,
    pub(crate) spec: Vec<Complex<f32>>,
    pub(crate) mag: Vec<f32>,
    pub(crate) phase: Vec<f32>,
    pub(crate) logmag: Vec<f32>,
    pub(crate) env: Vec<f32>,
    pub(crate) cep: Vec<Complex<f32>>,
    pub(crate) prev_phase: Vec<f32>,
    pub(crate) out_phase: Vec<f32>,
    pub(crate) lifter_len: usize,
    pub(crate) fs: u32,

    /// Pitch factor: output pitch = input pitch * ratio (1.0 = unchanged).
    pub pitch_ratio: f32,
    /// Formant factor: output formants = input formants * ratio (1.0 = unchanged).
    pub formant_ratio: f32,
    /// Detected fundamental of the most recent frame (Hz, 0 = unvoiced).
    pub last_f0: f32,
}

impl PitchShifter {
    pub fn new(fft_size: usize, hop: usize, sample_rate: u32) -> Self {
        debug_assert!(fft_size.is_multiple_of(hop));
        let frames = fft_size / hop;

        let mut window = vec![0.0; fft_size];
        for (i, w) in window.iter_mut().enumerate() {
            *w =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos();
        }

        // Steady-state OLA normalization (75% overlap: every sample is covered
        // by `frames` windows; sum of w^2 over those offsets).
        let mut norm = 0.0;
        for i in 0..frames {
            let idx = i * hop;
            norm += window[idx] * window[idx];
        }
        debug_assert!(norm > 0.0);

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);
        let env_ifft = planner.plan_fft_inverse(fft_size);
        let env_fft = planner.plan_fft_forward(fft_size);

        Self {
            n: fft_size,
            hop,
            window,
            norm,
            fft,
            ifft,
            env_ifft,
            env_fft,
            in_buf: Vec::new(),
            out_buf: vec![0.0; fft_size],
            spec: vec![Complex::new(0.0, 0.0); fft_size],
            mag: vec![0.0; fft_size / 2 + 1],
            phase: vec![0.0; fft_size / 2 + 1],
            logmag: vec![0.0; fft_size / 2 + 1],
            env: vec![0.0; fft_size / 2 + 1],
            cep: vec![Complex::new(0.0, 0.0); fft_size],
            prev_phase: vec![0.0; fft_size / 2 + 1],
            out_phase: vec![0.0; fft_size / 2 + 1],
            lifter_len: fft_size / 8,
            fs: sample_rate,
            pitch_ratio: 1.0,
            formant_ratio: 1.0,
            last_f0: 0.0,
        }
    }

    /// Stream `input` through the shifter, appending processed samples to
    /// `output` (same count in steady state, plus startup latency).
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        for &sample in input {
            self.in_buf.push(sample);
            if self.in_buf.len() == self.n {
                self.process_frame();
                self.in_buf.drain(..self.hop);
                output.extend_from_slice(&self.out_buf[..self.hop]);
                self.out_buf.drain(..self.hop);
                self.out_buf.resize(self.n, 0.0);
            }
        }
    }
}
