mod audio;
mod config;
mod dsp;
mod ui;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "vois",
    version,
    about = "Real-time voice changer written in pure Rust"
)]
pub struct Cli {
    #[arg(short = 'i', long, help = "Input (microphone) device name")]
    pub input: Option<String>,

    #[arg(short = 'o', long, help = "Output device name")]
    pub output: Option<String>,

    #[arg(
        short = 's',
        long,
        help = "Sample rate in Hz (must be supported by both devices)"
    )]
    pub sample_rate: Option<u32>,

    #[arg(long, help = "List available audio devices and exit")]
    pub list: bool,

    #[arg(
        short = 'p',
        long,
        default_value = "Clean",
        help = "Starting preset name"
    )]
    pub preset: String,

    #[arg(
        long,
        help = "Record processed audio to a WAV file (also toggle with R in the UI)"
    )]
    pub record: Option<String>,

    #[arg(long, default_value_t = 0.0, help = "Initial pitch shift in semitones")]
    pub pitch: f32,

    #[arg(
        long,
        default_value_t = 0.0,
        help = "Initial formant shift in semitones"
    )]
    pub formant: f32,

    #[arg(
        long,
        default_value_t = -100.0,
        help = "Noise gate threshold in dB (default -100 = off)"
    )]
    pub gate: f32,

    #[arg(long, default_value_t = 1.0, help = "Output gain (1.0 = unity)")]
    pub gain: f32,

    #[arg(long, help = "Exit automatically after this many seconds")]
    pub run_seconds: Option<u32>,

    #[arg(
        long,
        default_value_t = 0.0,
        help = "Mix a test tone (Hz) into the input; 0 = off. Useful to hear presets without a mic"
    )]
    pub tone: f32,

    #[arg(long, help = "List available presets and exit")]
    pub list_presets: bool,

    #[arg(
        long,
        help = "Disable the built-in virtual mic (vois.rs); on by default"
    )]
    pub no_virtual_mic: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list {
        audio::devices::print_all()?;
        return Ok(());
    }

    if cli.list_presets {
        println!("Available presets: {}", config::AppConfig::preset_names());
        return Ok(());
    }

    if !cli.no_virtual_mic {
        match audio::virtual_mic::VirtualMic::setup() {
            Ok(Some(src)) => {
                println!(
                    "virtual mic ready: use \"{src}\" (or \"vois.rs.monitor\") as your microphone"
                )
            }
            Ok(None) => println!("virtual mic setup failed: no source reported"),
            Err(e) => println!("warning: virtual mic setup failed: {e:#}"),
        }
    }

    let cfg = config::AppConfig::from_cli(&cli);
    audio::run(&cfg)
}
