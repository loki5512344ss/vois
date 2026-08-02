//! The DSP chain: one mono block through gate -> pitch/formant -> effects.

#[cfg(test)]
mod tests;
pub mod worker;

use crate::audio::record;
use crate::dsp::control::{Control, Mode, Stats};
use crate::dsp::formant::ratio_from_semitones;
use crate::dsp::fx::filter::NoiseGate;
use crate::dsp::fx::Effect;
use crate::dsp::preset::{build_effects, presets};
use crate::dsp::shift::PitchShifter;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

pub struct Processor {
    fs: u32,
    control: Arc<Mutex<Control>>,
    stats: Arc<Mutex<Stats>>,
    shifter: Option<PitchShifter>,
    shift_pending: Vec<f32>,
    /// Smoothed Auto-Tune correction in semitones.
    correction: f32,
    gate: NoiseGate,
    effects: Vec<Box<dyn Effect>>,
    last_preset: usize,
    recorder_tx: Option<Sender<Vec<f32>>>,
    recorder_handle: Option<JoinHandle<()>>,
}

impl Processor {
    pub fn new(fs: u32, control: Arc<Mutex<Control>>, stats: Arc<Mutex<Stats>>) -> Self {
        Self {
            fs,
            control,
            stats,
            shifter: None,
            shift_pending: Vec::new(),
            correction: 0.0,
            gate: NoiseGate::new(-60.0),
            effects: Vec::new(),
            last_preset: usize::MAX,
            recorder_tx: None,
            recorder_handle: None,
        }
    }

    /// Stop recording and wait for the WAV writer to finalize.
    pub(crate) fn stop_recording(&mut self) {
        self.recorder_tx = None;
        if let Some(handle) = self.recorder_handle.take() {
            let _ = handle.join();
        }
    }

    /// Process one fixed-size mono block through the whole chain.
    pub fn process_block(&mut self, mono: &mut [f32]) {
        if mono.is_empty() {
            return;
        }
        let t0 = Instant::now();

        let mut in_sum = 0.0;
        for &s in mono.iter() {
            in_sum += s * s;
        }
        let in_rms = (in_sum / mono.len() as f32).sqrt();

        let (
            preset_idx,
            pitch,
            formant,
            gate_db,
            gain,
            preset_gain,
            mute,
            record,
            mode,
            record_path,
            pitch_correct,
        ) = {
            let ctrl = match self.control.lock() {
                Ok(c) => c,
                Err(p) => p.into_inner(),
            };
            let preset = &presets()[ctrl.preset_idx.min(presets().len() - 1)];
            (
                ctrl.preset_idx.min(presets().len() - 1),
                preset.pitch_semitones + ctrl.pitch_delta,
                preset.formant_ratio * ratio_from_semitones(ctrl.formant_delta),
                // User-set gate wins; otherwise fall back to the preset's gate.
                if ctrl.gate_threshold_db > -90.0 {
                    ctrl.gate_threshold_db
                } else {
                    preset.gate_db
                },
                ctrl.gain,
                preset.gain,
                ctrl.mute,
                ctrl.record,
                ctrl.mode,
                ctrl.record_path.clone(),
                ctrl.pitch_correct,
            )
        };

        if preset_idx != self.last_preset {
            self.effects = build_effects(&presets()[preset_idx].effects, self.fs);
            self.last_preset = preset_idx;
        }

        let mut shifter_active = (pitch - 0.0).abs() > 0.05 || (formant - 1.0).abs() > 0.01;
        shifter_active |= pitch_correct;

        if mute {
            mono.fill(0.0);
        } else if mode == Mode::Passthrough {
            for s in mono.iter_mut() {
                *s *= gain;
            }
        } else {
            self.gate.set_threshold_db(gate_db);
            self.gate.process(mono);

            if shifter_active {
                self.shift(mono, pitch, formant, pitch_correct);
            }

            for effect in &mut self.effects {
                effect.process(mono);
            }

            for s in mono.iter_mut() {
                *s *= preset_gain * gain;
            }
        }

        self.handle_record(mono, record, record_path);
        self.update_stats(mono, in_rms, t0, shifter_active);
    }

    /// Pitch/formant shift with streaming OLA extraction.
    fn shift(&mut self, mono: &mut [f32], pitch: f32, formant: f32, pitch_correct: bool) {
        if self.shifter.is_none() {
            self.shifter = Some(PitchShifter::new(1024, 256, self.fs));
            // Prime with the vocoder's fixed latency so every input block
            // yields a full output block.
            self.shift_pending = vec![0.0; 1024 - 256];
        }
        let shifter = self.shifter.as_mut().unwrap();

        // Auto-Tune: glide the manual pitch toward the nearest semitone of
        // the detected input fundamental (smoothed so it doesn't wobble).
        if pitch_correct {
            let f0 = shifter.last_f0;
            if (70.0..1200.0).contains(&f0) {
                let current = 12.0 * (f0 / 440.0).log2();
                let target = current.round() - current;
                self.correction += (target - self.correction) * 0.25;
            }
        } else {
            self.correction = 0.0;
        }
        let effective = pitch + self.correction;

        shifter.pitch_ratio = 2f32.powf(effective / 12.0);
        shifter.formant_ratio = formant;
        let mut out = Vec::with_capacity(mono.len());
        shifter.process(mono, &mut out);

        // The vocoder is a streaming delay line: it may emit a different
        // number of samples than consumed. Buffer the excess and pull exactly
        // `mono.len()` samples out.
        self.shift_pending.extend(out);
        let take = mono.len().min(self.shift_pending.len());
        mono[..take].copy_from_slice(&self.shift_pending[..take]);
        mono[take..].fill(0.0);
        self.shift_pending.drain(..take);
    }

    fn handle_record(&mut self, mono: &[f32], record: bool, record_path: Option<String>) {
        if record {
            if self.recorder_tx.is_none() {
                if let Some(path) = record_path {
                    let (tx, handle) = record::spawn_writer(path, self.fs);
                    self.recorder_tx = Some(tx);
                    self.recorder_handle = Some(handle);
                }
            }
            if let Some(tx) = &self.recorder_tx {
                let _ = tx.send(mono.to_vec());
            }
        } else if self.recorder_tx.is_some() {
            self.stop_recording();
        }
    }

    fn update_stats(&mut self, mono: &[f32], in_rms: f32, t0: Instant, shifter_active: bool) {
        let mut out_sum = 0.0;
        for &s in mono.iter() {
            out_sum += s * s;
        }
        let out_rms = (out_sum / mono.len() as f32).sqrt();
        let elapsed = t0.elapsed().as_secs_f32();
        let block_dur = mono.len() as f32 / self.fs as f32;
        let cpu = if block_dur > 0.0 {
            elapsed / block_dur * 100.0
        } else {
            0.0
        };

        let mut st = match self.stats.lock() {
            Ok(s) => s,
            Err(p) => p.into_inner(),
        };
        st.input_rms = st.input_rms * 0.85 + in_rms * 0.15;
        st.output_rms = st.output_rms * 0.85 + out_rms * 0.15;
        st.peak_in = st.peak_in.max(in_rms);
        st.peak_out = st.peak_out.max(out_rms);
        st.blocks += 1;
        st.samples += mono.len() as u64;
        st.cpu_pct = st.cpu_pct * 0.9 + cpu * 0.1;
        // DSP latency estimate: block buffering + vocoder frame-fill.
        let latency_samples = if shifter_active { 768 } else { 0 } + 256;
        st.dsp_latency_ms = latency_samples as f32 / self.fs as f32 * 1000.0;
    }
}
