use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};

pub fn print_all() -> Result<()> {
    let host = cpal::default_host();

    println!("Input devices:");
    for device in host.input_devices()? {
        println!("  {}", device.name().unwrap_or_else(|_| "<unnamed>".into()));
    }

    println!("Output devices:");
    for device in host.output_devices()? {
        println!("  {}", device.name().unwrap_or_else(|_| "<unnamed>".into()));
    }

    Ok(())
}
