use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{
    Device, FromSample, Host, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    SupportedStreamConfig,
};
use ringbuf::traits::Producer;

pub fn find_input(host: &Host, name: Option<&str>) -> Result<Device> {
    match name {
        Some(name) => host
            .input_devices()
            .context("failed to enumerate input devices")?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("input device {name:?} not found")),
        None => host
            .default_input_device()
            .context("no default input device found"),
    }
}

/// Open the input stream. Captured samples are converted to `f32` (interleaved
/// frames) and pushed into `tx`.
pub fn start<P>(device: &Device, supported: &SupportedStreamConfig, tx: P) -> Result<Stream>
where
    P: Producer<Item = f32> + Send + 'static,
{
    let config: StreamConfig = supported.config();
    match supported.sample_format() {
        SampleFormat::F32 => build::<f32, P>(device, &config, tx),
        SampleFormat::F64 => build::<f64, P>(device, &config, tx),
        SampleFormat::I8 => build::<i8, P>(device, &config, tx),
        SampleFormat::I16 => build::<i16, P>(device, &config, tx),
        SampleFormat::I32 => build::<i32, P>(device, &config, tx),
        SampleFormat::I64 => build::<i64, P>(device, &config, tx),
        SampleFormat::U8 => build::<u8, P>(device, &config, tx),
        SampleFormat::U16 => build::<u16, P>(device, &config, tx),
        SampleFormat::U32 => build::<u32, P>(device, &config, tx),
        SampleFormat::U64 => build::<u64, P>(device, &config, tx),
        format => anyhow::bail!("unsupported input sample format: {format:?}"),
    }
}

fn build<T, P>(device: &Device, config: &StreamConfig, mut tx: P) -> Result<Stream>
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
