pub mod capture;
pub mod devices;
pub mod playback;
pub mod record;
pub mod virtual_mic;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig, SupportedStreamConfig, SupportedStreamConfigRange};
use ringbuf::traits::Split;
use ringbuf::HeapRb;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::AppConfig;
use crate::dsp;
use crate::ui;

/// Higher = better quality for our purposes.
fn format_rank(format: cpal::SampleFormat) -> u8 {
    match format {
        cpal::SampleFormat::F32 => 10,
        cpal::SampleFormat::I16 => 9,
        cpal::SampleFormat::I32 => 8,
        cpal::SampleFormat::F64 => 7,
        cpal::SampleFormat::U16 => 6,
        cpal::SampleFormat::I8 => 5,
        cpal::SampleFormat::U8 => 4,
        _ => 0,
    }
}

/// Find the best supported config with the exact sample rate (same channel
/// count as `channels`). This is how we force both devices onto one sample
/// rate while preferring a high-quality sample format.
fn pick_config(
    ranges: impl Iterator<Item = SupportedStreamConfigRange>,
    channels: u16,
    rate: SampleRate,
) -> Option<SupportedStreamConfig> {
    ranges
        .filter(|r| r.channels() == channels && r.try_with_sample_rate(rate).is_some())
        .max_by_key(|r| format_rank(r.sample_format()))
        .map(|r| r.with_sample_rate(rate))
}

pub fn run(cfg: &AppConfig) -> Result<()> {
    let host = cpal::default_host();

    let input_device = capture::find_input(&host, cfg.input.as_deref())?;
    let output_device = playback::find_output(&host, cfg.output.as_deref())?;

    let in_supported = input_device
        .default_input_config()
        .context("failed to read default input config")?;
    let in_channels = in_supported.channels();
    let out_default = output_device
        .default_output_config()
        .context("failed to read default output config")?;
    let out_channels = out_default.channels();

    // Negotiate one sample rate both devices support, preferring 48 kHz.
    let rate = if let Some(r) = cfg.sample_rate {
        SampleRate(r)
    } else {
        let in_ranges = input_device.supported_input_configs()?;
        let out_ranges = output_device.supported_output_configs()?;
        let in_ranges: Vec<_> = in_ranges.collect();
        let out_ranges: Vec<_> = out_ranges.collect();
        let mut chosen = in_supported.sample_rate();
        for r in [
            SampleRate(48000),
            SampleRate(44100),
            in_supported.sample_rate(),
        ] {
            if pick_config(in_ranges.iter().cloned(), in_channels, r).is_some()
                && pick_config(out_ranges.iter().cloned(), out_channels, r).is_some()
            {
                chosen = r;
                break;
            }
        }
        chosen
    };

    let in_supported = pick_config(input_device.supported_input_configs()?, in_channels, rate)
        .with_context(|| format!("input device does not support sample rate {} Hz", rate.0))?;

    let out_supported = pick_config(
        output_device.supported_output_configs()?,
        out_channels,
        rate,
    )
    .with_context(|| {
        format!(
            "output device does not support sample rate {} Hz; try `--sample-rate`",
            rate.0
        )
    })?;

    let in_cfg: StreamConfig = in_supported.config();
    let out_cfg: StreamConfig = out_supported.config();

    println!(
        "input  : {} @ {} Hz, {:?}, {} ch",
        input_device.name().unwrap_or_else(|_| "<unnamed>".into()),
        in_cfg.sample_rate.0,
        in_supported.sample_format(),
        in_cfg.channels,
    );
    println!(
        "output : {} @ {} Hz, {:?}, {} ch",
        output_device.name().unwrap_or_else(|_| "<unnamed>".into()),
        out_cfg.sample_rate.0,
        out_supported.sample_format(),
        out_cfg.channels,
    );

    let ring_cap = (rate.0 as usize * 2).max(8192);
    let (in_prod, in_cons) = HeapRb::<f32>::new(ring_cap).split();
    let (out_prod, out_cons) = HeapRb::<f32>::new(ring_cap).split();

    // Virtual mic: keep `vois.rs` as the default sink for the whole run so the
    // app's output lands in the virtual mic. A helper thread parks other
    // apps' streams back on the real sink so their audio doesn't leak into the
    // mic. The default sink is restored on exit.
    let prev_sink = if cfg.virtual_mic {
        virtual_mic::VirtualMic::setup()?;
        virtual_mic::VirtualMic::get_default_sink()
    } else {
        None
    };
    if prev_sink.is_some() {
        virtual_mic::VirtualMic::set_default_sink("vois.rs");
    }

    let input_stream = capture::start(&input_device, &in_supported, in_prod)?;
    let output_stream = playback::start(&output_device, &out_supported, out_cons)?;

    let in_ch = in_cfg.channels as usize;
    let out_ch = out_cfg.channels as usize;

    let control = Arc::new(Mutex::new(cfg.to_control()));
    let stats = Arc::new(Mutex::new(dsp::stats::Stats::default()));
    let worker_control = Arc::clone(&control);
    let worker_stats = Arc::clone(&stats);
    let worker = thread::spawn(move || {
        dsp::chain::run_worker(
            in_cons,
            out_prod,
            in_ch,
            out_ch,
            rate.0,
            worker_control,
            worker_stats,
        )
    });

    input_stream
        .play()
        .context("failed to start input stream")?;
    output_stream
        .play()
        .context("failed to start output stream")?;
    let cleanup = if let Some(prev) = prev_sink {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handle = virtual_mic::VirtualMic::keep_others_off(Arc::clone(&stop), prev.clone());
        let _sig = virtual_mic::VirtualMic::restore_on_signal(prev.clone());
        Some((stop, handle, prev))
    } else {
        None
    };

    let info = ui::SessionInfo {
        input: input_device.name().unwrap_or_else(|_| "<unnamed>".into()),
        output: output_device.name().unwrap_or_else(|_| "<unnamed>".into()),
        sample_rate: rate.0,
        in_ch,
        out_ch,
        format_in: format!("{:?}", in_supported.sample_format()),
        format_out: format!("{:?}", out_supported.sample_format()),
    };

    // Keep the streams alive while the UI blocks.
    let _streams = (input_stream, output_stream);

    ui::tui::run(control, stats, info);

    if let Some((stop, handle, prev)) = cleanup {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
        virtual_mic::VirtualMic::set_default_sink(&prev);
    }

    println!("waiting for DSP worker to finish...");
    let _ = worker.join();

    // cpal's ALSA teardown can panic on PipeWire when the stream drop races
    // with the worker thread closing its self-pipe. The process is about to
    // exit anyway, so leak the streams instead of dropping them.
    std::mem::forget(_streams);
    println!("done.");
    Ok(())
}
