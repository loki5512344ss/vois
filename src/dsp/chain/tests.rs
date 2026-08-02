use super::Processor;
use crate::dsp::control::{Control, Stats};
use std::sync::{Arc, Mutex};

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

#[test]
fn girl_preset_produces_energy_on_sine() {
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
    let control = Arc::new(Mutex::new(Control {
        preset_idx: 1,
        ..Control::default()
    }));
    let mut p = Processor::new(48000, control, Arc::new(Mutex::new(Stats::default())));
    let mut state = 0x12345678u32;
    let mut input = Vec::new();
    for _ in 0..96000 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        input.push(state as f32 / u32::MAX as f32 * 2.0 - 1.0);
    }
    for v in input.iter_mut() {
        *v *= 0.01;
    }
    let out = feed_blocks(&mut p, &input);
    let r = rms(&out[4000..]);
    assert!(r.is_finite() && r > 0.0005, "girl noise rms too low: {r}");
}
