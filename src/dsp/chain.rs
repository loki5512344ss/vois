use crate::audio::record;
use crate::dsp::effects::Effect;
use crate::dsp::formant::ratio_from_semitones;
use crate::dsp::gate::NoiseGate;
use crate::dsp::pitch::PitchShifter;
use crate::dsp::presets::{build_effects, presets};
use crate::dsp::stats::Stats;
use ringbuf::traits::{Consumer, Producer};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Processing mode, switchable at runtime (A/B compare).
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// Normal processing through the DSP chain.
    Live,
    /// Raw input straight to output (bypass) for A/B comparison.
    Passthrough,
}

impl Mode {
    pub fn toggle(&mut self) {
        *self = match self {
            Mode::Live => Mode::Passthrough,
            Mode::Passthrough => Mode::Live,
        };
    }
}

/// Live-adjustable state, shared between the UI and the DSP worker.
pub struct Control {
    pub preset_idx: usize,
    /// Manual pitch adjustment added to the preset value (semitones).
    pub pitch_delta: f32,
    /// Manual formant adjustment added to the preset value (semitones).
    pub formant_delta: f32,
    pub gate_threshold_db: f32,
    pub gain: f32,
    pub mute: bool,
    pub record: bool,
    pub record_path: Option<String>,
    pub run_seconds: Option<u32>,
    /// Test tone frequency (Hz) mixed into the input; 0 = off.
    pub tone_hz: f32,
    /// Snap the voice to the nearest semitone (Auto-Tune style).
    pub pitch_correct: bool,
    pub mode: Mode,
    pub quit: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            preset_idx: 0,
            pitch_delta: 0.0,
            formant_delta: 0.0,
            gate_threshold_db: -100.0,
            gain: 1.0,
            mute: false,
            record: false,
            record_path: None,
            run_seconds: None,
            tone_hz: 0.0,
            pitch_correct: false,
            mode: Mode::Live,
            quit: false,
        }
    }
}

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
    fn stop_recording(&mut self) {
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

        // Input level (pre-processing).
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
            // A/B compare: raw input, only gain.
            for s in mono.iter_mut() {
                *s *= gain;
            }
        } else {
            self.gate.set_threshold_db(gate_db);
            self.gate.process(mono);

            if shifter_active {
                if self.shifter.is_none() {
                    self.shifter = Some(PitchShifter::new(1024, 256, self.fs));
                    // Prime the extraction buffer with the vocoder's fixed latency
                    // (n - hop) so every input block yields a full output block.
                    self.shift_pending = vec![0.0; 1024 - 256];
                }
                let shifter = self.shifter.as_mut().unwrap();

                // Auto-Tune: glide the manual pitch toward the nearest semitone
                // of the detected input fundamental (smoothed so it doesn't
                // wobble). Detection happens on the input frame, so the
                // correction applies from the next block onward.
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

                // The vocoder is a streaming delay line: per call it may emit a
                // different number of samples than it consumed. Keep the excess
                // in `shift_pending` and pull exactly `mono.len()` samples out,
                // so the rest of the chain sees constant block sizes.
                self.shift_pending.extend(out);
                let take = mono.len().min(self.shift_pending.len());
                mono[..take].copy_from_slice(&self.shift_pending[..take]);
                mono[take..].fill(0.0);
                self.shift_pending.drain(..take);
            }

            for effect in &mut self.effects {
                effect.process(mono);
            }

            for s in mono.iter_mut() {
                *s *= preset_gain * gain;
            }
        }

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

        // Output level + runtime stats.
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
        // DSP latency estimate: block buffering + vocoder frame-fill when active.
        let latency_samples = if shifter_active { 768 } else { 0 } + 256;
        st.dsp_latency_ms = latency_samples as f32 / self.fs as f32 * 1000.0;
    }
}

/// DSP worker thread: pulls captured audio, runs the chain, pushes the result
/// to the output.
pub fn run_worker<I, O>(
    mut input: I,
    mut output: O,
    in_ch: usize,
    out_ch: usize,
    fs: u32,
    control: Arc<Mutex<Control>>,
    stats: Arc<Mutex<Stats>>,
) where
    I: Consumer<Item = f32> + Send + 'static,
    O: Producer<Item = f32> + Send + 'static,
{
    let mut processor = Processor::new(fs, control, stats);
    let mut scratch = vec![0f32; 8192];
    let mut acc = Vec::with_capacity(8192);
    let mut block = vec![0f32; 256];
    let mut out = Vec::with_capacity(8192);
    let mut total_blocks: u64 = 0;
    let mut total_samples: u64 = 0;
    let mut tone_phase: f32 = 0.0;

    loop {
        if processor.control.lock().map(|c| c.quit).unwrap_or(false) {
            break;
        }

        let n = input.pop_slice(&mut scratch);
        if n == 0 {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        total_samples += n as u64;

        let frames = n / in_ch.max(1);
        for f in 0..frames {
            let mut mix = 0.0;
            for c in 0..in_ch {
                mix += scratch[f * in_ch + c];
            }
            acc.push(mix / in_ch as f32);
        }

        // Optional test tone injected before the DSP chain.
        let tone_hz = processor.control.lock().map(|c| c.tone_hz).unwrap_or(0.0);
        if tone_hz > 0.0 {
            let start = acc.len().saturating_sub(frames);
            for v in acc.iter_mut().skip(start) {
                *v += 0.5 * (2.0 * std::f32::consts::PI * tone_phase).sin();
                tone_phase += tone_hz / fs as f32;
                if tone_phase >= 1.0 {
                    tone_phase -= 1.0;
                }
            }
        }

        // Process in fixed-size blocks (multiple of the vocoder hop) so the
        // pitch shifter always yields a full block. Smaller blocks cut the
        // worst-case buffering latency; the shifter stream is unchanged.
        while acc.len() >= 256 {
            block.copy_from_slice(&acc[..256]);
            acc.drain(..256);
            total_blocks += 1;

            processor.process_block(&mut block);

            out.clear();
            for v in &block {
                for _ in 0..out_ch {
                    out.push(*v);
                }
            }
            let _ = output.push_slice(&out);
        }
    }

    eprintln!("dsp: processed {total_blocks} blocks / {total_samples} samples");
    processor.stop_recording();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        let s: f32 = x.iter().map(|v| v * v).sum();
        (s / x.len().max(1) as f32).sqrt()
    }

    fn dominant_hz(x: &[f32], fs: u32) -> f32 {
        use rustfft::num_complex::Complex;
        use rustfft::FftPlanner;
        let n = 4096;
        if x.len() < n {
            return 0.0;
        }
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let mut buf: Vec<Complex<f32>> = x[..n].iter().map(|&s| Complex::new(s, 0.0)).collect();
        fft.process(&mut buf);
        let bins = n / 2;
        let mags: Vec<f32> = (0..bins).map(|b| buf[b].norm()).collect();
        let mut best = (0usize, 0.0f32);
        for b in 20..(bins / 4) {
            let mut s = 0.0;
            for h in 1..=4 {
                if b * h < bins {
                    s += mags[b * h];
                }
            }
            if s > best.1 {
                best = (b, s);
            }
        }
        fs as f32 * best.0 as f32 / n as f32
    }

    fn preset_control(idx: usize) -> Arc<Mutex<Control>> {
        Arc::new(Mutex::new(Control {
            preset_idx: idx,
            ..Control::default()
        }))
    }

    #[test]
    fn girl_preset_produces_energy_on_sine() {
        // Clean preset + manual pitch delta = pitch-only (no formant shift).
        let control = Arc::new(Mutex::new(Control {
            preset_idx: 0,
            pitch_delta: 5.0,
            ..Control::default()
        }));
        let mut p = Processor::new(48000, control, Arc::new(Mutex::new(Stats::default())));
        // Harmonic-rich tone (closer to voice than a pure sine).
        let input: Vec<f32> = (0..96000)
            .map(|i| {
                let t = 220.0 * i as f32 / 48000.0;
                let mut s = 0.0;
                for h in 1..=12 {
                    s += (2.0 * std::f32::consts::PI * h as f32 * t).sin() / h as f32;
                }
                s
            })
            .collect();
        let out = feed_blocks(&mut p, &input);
        let r = rms(&out[4000..]);
        assert!(r.is_finite() && r > 0.05, "girl sine rms too low: {r}");

        // Pitch must actually shift: 220 Hz * 2^(5/12) ~= 293.7 Hz.
        let est = dominant_hz(&out[20000..96000], 48000);
        assert!(
            (est - 293.7).abs() < 12.0,
            "girl pitch expected ~293.7 Hz, got {est:.1} Hz"
        );
    }

    #[test]
    fn girl_preset_produces_energy_on_noise() {
        let control = preset_control(1);
        let mut p = Processor::new(48000, control, Arc::new(Mutex::new(Stats::default())));
        let mut state = 0x12345678u32;
        let mut input = Vec::new();
        for _ in 0..96000 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            input.push(state as f32 / u32::MAX as f32 * 2.0 - 1.0);
        }
        // Scale to ~mic-hiss level.
        for v in input.iter_mut() {
            *v *= 0.01;
        }
        let out = feed_blocks(&mut p, &input);
        let r = rms(&out[4000..]);
        assert!(r.is_finite() && r > 0.0005, "girl noise rms too low: {r}");
    }

    /// Feed input through the processor in the same fixed-size blocks the
    /// real worker uses, collecting the output.
    fn feed_blocks(p: &mut Processor, input: &[f32]) -> Vec<f32> {
        let mut out_all = Vec::with_capacity(input.len());
        for chunk in input.chunks(256) {
            let mut block = chunk.to_vec();
            block.resize(256, 0.0);
            p.process_block(&mut block);
            out_all.extend_from_slice(&block[..chunk.len()]);
        }
        out_all
    }
}
