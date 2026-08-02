//! Audio device selection and stream creation (capture + playback).

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{
    Device, FromSample, Host, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    SupportedStreamConfig,
};
use ringbuf::traits::{Consumer, Producer};

const PREFERRED_INPUTS: [&str; 3] = ["pipewire", "pulse", "default"];
const PREFERRED_OUTPUTS: [&str; 3] = ["pipewire", "pulse", "default"];

fn pick(
    devices: impl Iterator<Item = Device>,
    preferred: &[&str],
    default: Option<Device>,
) -> Option<Device> {
    let mut devices = devices;
    devices
        .find(|d| {
            d.name()
                .map(|n| preferred.contains(&n.as_str()))
                .unwrap_or(false)
        })
        .or(default)
}

pub fn find_input(host: &Host, name: Option<&str>) -> Result<Device> {
    match name {
        Some(name) => host
            .input_devices()
            .context("failed to enumerate input devices")?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("input device {name:?} not found")),
        None => pick(
            host.input_devices()
                .context("failed to enumerate input devices")?,
            &PREFERRED_INPUTS,
            host.default_input_device(),
        )
        .context("no default input device found"),
    }
}

pub fn find_output(host: &Host, name: Option<&str>) -> Result<Device> {
    match name {
        Some(name) => host
            .output_devices()
            .context("failed to enumerate output devices")?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("output device {name:?} not found")),
        None => pick(
            host.output_devices()
                .context("failed to enumerate output devices")?,
            &PREFERRED_OUTPUTS,
            host.default_output_device(),
        )
        .context("no default output device found"),
    }
}

/// Open the input stream. Samples are converted to `f32` (interleaved frames)
/// and pushed into `tx`.
pub fn start_input<P>(device: &Device, supported: &SupportedStreamConfig, tx: P) -> Result<Stream>
where
    P: Producer<Item = f32> + Send + 'static,
{
    let config: StreamConfig = supported.config();
    match supported.sample_format() {
        SampleFormat::F32 => build_input::<f32, P>(device, &config, tx),
        SampleFormat::F64 => build_input::<f64, P>(device, &config, tx),
        SampleFormat::I8 => build_input::<i8, P>(device, &config, tx),
        SampleFormat::I16 => build_input::<i16, P>(device, &config, tx),
        SampleFormat::I32 => build_input::<i32, P>(device, &config, tx),
        SampleFormat::I64 => build_input::<i64, P>(device, &config, tx),
        SampleFormat::U8 => build_input::<u8, P>(device, &config, tx),
        SampleFormat::U16 => build_input::<u16, P>(device, &config, tx),
        SampleFormat::U32 => build_input::<u32, P>(device, &config, tx),
        SampleFormat::U64 => build_input::<u64, P>(device, &config, tx),
        format => anyhow::bail!("unsupported input sample format: {format:?}"),
    }
}

fn build_input<T, P>(device: &Device, config: &StreamConfig, mut tx: P) -> Result<Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
    P: Producer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;
    let mut frame = vec![0f32; channels];
    let err_fn = |err| eprintln!("capture stream error: {err}");

    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                for chunk in data.chunks(channels) {
                    for (slot, sample) in frame.iter_mut().zip(chunk) {
                        *slot = f32::from_sample(*sample);
                    }
                    let _ = tx.push_slice(&frame);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| anyhow::anyhow!("failed to build input stream: {e}"))
}

/// Open the output stream. Pulls interleaved `f32` frames from `rx`; fills
/// with silence when nothing is ready.
pub fn start_output<C>(device: &Device, supported: &SupportedStreamConfig, rx: C) -> Result<Stream>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let config: StreamConfig = supported.config();
    match supported.sample_format() {
        SampleFormat::F32 => build_output::<f32, C>(device, &config, rx),
        SampleFormat::F64 => build_output::<f64, C>(device, &config, rx),
        SampleFormat::I8 => build_output::<i8, C>(device, &config, rx),
        SampleFormat::I16 => build_output::<i16, C>(device, &config, rx),
        SampleFormat::I32 => build_output::<i32, C>(device, &config, rx),
        SampleFormat::I64 => build_output::<i64, C>(device, &config, rx),
        SampleFormat::U8 => build_output::<u8, C>(device, &config, rx),
        SampleFormat::U16 => build_output::<u16, C>(device, &config, rx),
        SampleFormat::U32 => build_output::<u32, C>(device, &config, rx),
        SampleFormat::U64 => build_output::<u64, C>(device, &config, rx),
        format => anyhow::bail!("unsupported output sample format: {format:?}"),
    }
}

fn build_output<T, C>(device: &Device, config: &StreamConfig, mut rx: C) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
    C: Consumer<Item = f32> + Send + 'static,
{
    let mut scratch = vec![0f32; 2048];
    let err_fn = |err| eprintln!("playback stream error: {err}");

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut written = 0usize;
                while written < data.len() {
                    let want = (data.len() - written).min(scratch.len());
                    let n = rx.pop_slice(&mut scratch[..want]);
                    if n == 0 {
                        data[written..].fill(T::from_sample(0.0f32));
                        break;
                    }
                    let out = &mut data[written..written + n];
                    for (o, i) in out.iter_mut().zip(&scratch[..n]) {
                        *o = T::from_sample(*i);
                    }
                    written += n;
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| anyhow::anyhow!("failed to build output stream: {e}"))
}
