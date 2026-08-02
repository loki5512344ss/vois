use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{
    Device, FromSample, Host, SampleFormat, SizedSample, Stream, StreamConfig,
    SupportedStreamConfig,
};
use ringbuf::traits::Consumer;

pub fn find_output(host: &Host, name: Option<&str>) -> Result<Device> {
    match name {
        Some(name) => host
            .output_devices()
            .context("failed to enumerate output devices")?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("output device {name:?} not found")),
        None => host
            .default_output_device()
            .context("no default output device found"),
    }
}

/// Open the output stream. Pulls interleaved `f32` frames from `rx`; fills
/// with silence when the DSP worker has nothing ready yet.
pub fn start<C>(device: &Device, supported: &SupportedStreamConfig, rx: C) -> Result<Stream>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let config: StreamConfig = supported.config();
    match supported.sample_format() {
        SampleFormat::F32 => build::<f32, C>(device, &config, rx),
        SampleFormat::F64 => build::<f64, C>(device, &config, rx),
        SampleFormat::I8 => build::<i8, C>(device, &config, rx),
        SampleFormat::I16 => build::<i16, C>(device, &config, rx),
        SampleFormat::I32 => build::<i32, C>(device, &config, rx),
        SampleFormat::I64 => build::<i64, C>(device, &config, rx),
        SampleFormat::U8 => build::<u8, C>(device, &config, rx),
        SampleFormat::U16 => build::<u16, C>(device, &config, rx),
        SampleFormat::U32 => build::<u32, C>(device, &config, rx),
        SampleFormat::U64 => build::<u64, C>(device, &config, rx),
        format => anyhow::bail!("unsupported output sample format: {format:?}"),
    }
}

fn build<T, C>(device: &Device, config: &StreamConfig, mut rx: C) -> Result<Stream>
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
                    for (out_sample, in_sample) in out.iter_mut().zip(&scratch[..n]) {
                        *out_sample = T::from_sample(*in_sample);
                    }
                    written += n;
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| anyhow::anyhow!("failed to build output stream: {e}"))
}
