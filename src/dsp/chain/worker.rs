//! The DSP worker thread: pulls captured audio, runs the chain, pushes output.

use super::Processor;
use crate::dsp::control::{Control, Stats};
use ringbuf::traits::{Consumer, Producer};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
        // pitch shifter always yields a full block.
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
