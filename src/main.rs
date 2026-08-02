mod audio;
mod config;
mod dsp;
mod ui;

use anyhow::Result;
use clap::Parser;
use config::FileConfig;

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

    #[arg(short = 'p', long, help = "Preset name (config/default: Clean)")]
    pub preset: Option<String>,

    #[arg(long, help = "Record processed audio to a WAV file")]
    pub record: Option<String>,

    #[arg(long, help = "Initial pitch shift in semitones")]
    pub pitch: Option<f32>,

    #[arg(long, help = "Initial formant shift in semitones")]
    pub formant: Option<f32>,

    #[arg(long, help = "Noise gate threshold in dB (default -100 = off)")]
    pub gate: Option<f32>,

    #[arg(long, help = "Output gain (1.0 = unity)")]
    pub gain: Option<f32>,

    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        help = "Snap pitch to the nearest semitone (Auto-Tune)"
    )]
    pub pitch_correct: Option<bool>,

    #[arg(long, help = "Exit automatically after this many seconds")]
    pub run_seconds: Option<u32>,

    #[arg(
        long,
        help = "Mix a test tone (Hz) into the input; 0 = off. Useful to hear presets without a mic"
    )]
    pub tone: Option<f32>,

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

    let file = FileConfig::load();
    dsp::preset::init_custom(file.to_presets());

    if cli.list {
        audio::print_all()?;
        return Ok(());
    }

    if cli.list_presets {
        println!("Available presets: {}", config::AppConfig::preset_names());
        println!(
            "Config file: {} (create it to set defaults and custom presets)",
            FileConfig::config_path().display()
        );
        return Ok(());
    }

    let cfg = config::AppConfig::from_cli(&cli);

    if cfg.virtual_mic {
        match audio::virtual_mic::VirtualMic::setup() {
            Ok(Some(src)) => {
                println!(
                    "virtual mic ready: use \"{src}\" (or \"vois.rs.monitor\") as your microphone"
                )
            }
            Ok(None) => println!(
                "virtual mic not available on this platform: point -o at a virtual cable (e.g. VB-CABLE \"CABLE Input\") and select it in your voice app"
            ),
            Err(e) => println!("warning: virtual mic setup failed: {e:#}"),
        }
    }

    audio::run(&cfg)
}
