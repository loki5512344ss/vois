//! Latency + throughput probe for the vocoder DSP.
//!
//! This is a dev-only example, not part of the app. `vois` is a binary-only
//! crate (DSP lives behind private `mod dsp` in `main.rs`), so we pull in the
//! real DSP sources with `#[path]` and compile them into this example. No new
//! dependencies are needed: `rustfft` and `anyhow` are already in Cargo.toml.

// The example only exercises `Processor` + `PitchShifter`; the rest of the
// worker/Control surface compiled in from `dsp::chain` is intentionally unused
// here, so silence dead-code warnings for this example target only.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;

/// Minimal stand-in for `crate::audio::record` so `dsp::chain` compiles here.
mod audio {
    pub mod record {
        use std::sync::mpsc;
        use std::thread;

        pub fn spawn_writer(
            _path: String,
            _fs: u32,
        ) -> (mpsc::Sender<Vec<f32>>, thread::JoinHandle<()>) {
            let (tx, rx) = mpsc::channel();
            let handle = thread::spawn(move || while rx.recv().is_ok() {});
            (tx, handle)
        }
    }
}

#[path = "../src/dsp/mod.rs"]
mod dsp;

use dsp::chain::{Control, Processor};
use dsp::pitch::PitchShifter;
use dsp::stats::Stats;

const FS: u32 = 48000;

/// Harmonic-rich saw (like the unit tests use): narrow pitch lines + broad
/// envelope, closer to a voice than a pure sine.
fn saw(freq: f32, fs: u32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| {
            let t = freq * i as f32 / fs as f32;
            let mut s = 0.0;
            for h in 1..=12 {
                s += ((2.0 * std::f32::consts::PI * h as f32 * t).sin()) / h as f32;
            }
            s
        })
        .collect()
}

/// Feed `input` through the processor in the same fixed-size blocks the DSP
/// worker uses, collecting the output.
fn feed_blocks(p: &mut Processor, input: &[f32], block: usize) -> Vec<f32> {
    let mut out_all = Vec::with_capacity(input.len());
    for chunk in input.chunks(block) {
        let mut b = chunk.to_vec();
        b.resize(block, 0.0);
        p.process_block(&mut b);
        out_all.extend_from_slice(&b[..chunk.len()]);
    }
    out_all
}

/// CPU time needed to process one second of audio (in ms), measured over 10 s
/// of a 220 Hz saw fed in `block`-sized chunks. Preset "Girl / Anime" (pitch
/// +5 st, formant x1.25) exercises the heaviest vocoder path.
fn throughput(block: usize) -> f64 {
    let control = Arc::new(Mutex::new(Control {
        preset_idx: 1,
        ..Control::default()
    }));
    let mut p = Processor::new(FS, control, Arc::new(Mutex::new(Stats::default())));
    let audio = saw(220.0, FS, 10 * FS as usize);
    let t0 = Instant::now();
    let out = feed_blocks(&mut p, &audio, block);
    let dt = t0.elapsed();
    assert_eq!(out.len(), audio.len());
    dt.as_secs_f64() / 10.0 * 1000.0
}

/// Pitch-shifter group delay: feed a sharp transient, report the first output
/// sample above `thresh_frac` of the output peak. A unit impulse at sample 0
/// is annihilated by the Hann window (w[0] == 0 exactly), so the transient is
/// placed mid-frame where the window is at its peak.
fn group_delay(n: usize, hop: usize, impulse_at: usize, thresh_frac: f32) -> Option<(usize, f32)> {
    let mut sh = PitchShifter::new(n, hop, FS);
    sh.pitch_ratio = 1.0;
    let mut input = vec![0.0f32; 2 * n];
    input[impulse_at] = 1.0;
    let mut out = Vec::new();
    sh.process(&input, &mut out);
    let peak = out.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
    let thr = peak * thresh_frac;
    out.iter().position(|&v| v.abs() > thr).map(|i| (i, peak))
}

/// Practical onset delay through the whole chain (what a user would hear):
/// feed a sine burst that starts after a quiet lead-in and measure the gap
/// between the input onset and the first output sample above a threshold.
fn chain_onset_delay(block: usize, n: usize) -> Option<usize> {
    let control = Arc::new(Mutex::new(Control {
        preset_idx: 0,
        pitch_delta: 5.0,
        ..Control::default()
    }));
    let mut p = Processor::new(FS, control, Arc::new(Mutex::new(Stats::default())));

    let lead = 2 * n;
    let burst_len = 2048;
    let mut input = vec![0.0f32; lead + burst_len];
    for (i, s) in input.iter_mut().enumerate().skip(lead) {
        let t = 220.0 * (i - lead) as f32 / FS as f32;
        *s = (2.0 * std::f32::consts::PI * t).sin();
    }

    let out = feed_blocks(&mut p, &input, block);
    let peak = out[lead..].iter().fold(0.0f32, |a, &v| a.max(v.abs()));
    let thr = peak * 1e-2;
    out.iter().position(|&v| v.abs() > thr).map(|i| i - lead)
}

/// Sanity check: feeding the same audio at block=1024 vs block=256 must
/// produce (nearly) identical output — the block-size change only reduces
/// buffering latency, not audio content.
fn block_size_identity() {
    let saw2 = saw(220.0, FS, 2 * FS as usize);
    let mk = || {
        Arc::new(Mutex::new(Control {
            preset_idx: 1,
            ..Control::default()
        }))
    };
    let mut a = Processor::new(FS, mk(), Arc::new(Mutex::new(Stats::default())));
    let mut b = Processor::new(FS, mk(), Arc::new(Mutex::new(Stats::default())));
    let oa = feed_blocks(&mut a, &saw2, 1024);
    let ob = feed_blocks(&mut b, &saw2, 256);
    let max_diff = oa
        .iter()
        .zip(ob.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    println!("block 1024 vs 256 output: max |diff| = {max_diff:.3e}");
}

fn ms(samples: usize) -> f64 {
    samples as f64 / FS as f64 * 1000.0
}

fn main() -> Result<()> {
    let audio_seconds = 10.0;
    let (n, hop) = (1024usize, 256usize);

    println!("=== vois DSP latency probe (fs = {FS} Hz) ===");

    // a) Throughput.
    for block in [1024usize, 256usize] {
        let cpu_ms = throughput(block);
        println!(
            "throughput block={block:>4}: {cpu_ms:7.2} ms CPU per 1s audio \
             ({:5.1}% of one core, {:.1}x realtime)",
            cpu_ms / 10.0,
            audio_seconds / (cpu_ms / 1000.0)
        );
    }

    // b) Pitch-shifter group delay (impulse mid-frame, ratio = 1.0).
    let (gd, peak) = group_delay(n, hop, n / 2, 0.1).expect("no impulse response found");
    println!(
        "shifter group delay (n={n}, hop={hop}, impulse@{}, >10% of peak {:.2e}): \
         {gd} samples = {:.2} ms",
        n / 2,
        peak,
        ms(gd)
    );

    // Frame-fill latency: the shifter needs `n` samples before it can emit its
    // first hop, i.e. output lags input by n - hop in the stream.
    println!(
        "  streaming frame-fill latency n - hop = {} samples = {:.2} ms",
        n - hop,
        ms(n - hop)
    );

    // c) End-to-end estimate = measured group delay + fixed prime + block
    // buffering, for the current settings.
    let prime = n - hop;
    for block in [1024usize, 256usize] {
        let total = gd + prime + block;
        println!(
            "end-to-end (block={block:>4}): group delay {gd} + prime {prime} + buffering {block} \
             = {total} samples = {ms:.2} ms",
            ms = ms(total)
        );
    }

    // Chain onset delay is the number users would actually feel.
    for block in [1024usize, 256usize] {
        if let Some(d) = chain_onset_delay(block, n) {
            println!(
                "chain onset delay (block={block:>4}): {d} samples = {:.2} ms after burst start",
                ms(d)
            );
        }
    }

    block_size_identity();

    Ok(())
}
