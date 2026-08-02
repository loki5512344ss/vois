use crate::dsp::chain::Control;
use crate::dsp::presets::{presets, CustomPresetCfg, Preset};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Settings loaded from `~/.config/vois/config.toml`.
#[derive(Deserialize, Default)]
pub struct FileConfig {
    pub input: Option<String>,
    pub output: Option<String>,
    pub sample_rate: Option<u32>,
    pub preset: Option<String>,
    pub record: Option<String>,
    pub pitch: Option<f32>,
    pub formant: Option<f32>,
    pub gate: Option<f32>,
    pub gain: Option<f32>,
    pub tone: Option<f32>,
    pub virtual_mic: Option<bool>,
    pub pitch_correct: Option<bool>,
    pub custom_presets: Option<HashMap<String, CustomPresetCfg>>,
}

impl FileConfig {
    pub fn config_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".config"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            });
        base.join("vois").join("config.toml")
    }

    /// Load the config file if it exists; any parse error is ignored (treated
    /// as an empty config so the app still starts).
    pub fn load() -> Self {
        match std::fs::read_to_string(Self::config_path()) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Convert custom presets from the file into `Preset` objects.
    pub fn to_presets(&self) -> Vec<Preset> {
        self.custom_presets
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(name, c)| Preset {
                        name: name.clone(),
                        pitch_semitones: c.pitch.unwrap_or(0.0),
                        formant_ratio: c.formant.unwrap_or(1.0),
                        effects: c.effects.clone().unwrap_or_default(),
                        gain: 1.0,
                        gate_db: c.gate_db.unwrap_or(-100.0),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub struct AppConfig {
    pub input: Option<String>,
    pub output: Option<String>,
    pub sample_rate: Option<u32>,
    pub preset: String,
    pub record: Option<String>,
    pub pitch: f32,
    pub formant: f32,
    pub gate: f32,
    pub gain: f32,
    pub run_seconds: Option<u32>,
    pub tone: f32,
    pub virtual_mic: bool,
    pub pitch_correct: bool,
}

impl AppConfig {
    /// Merge CLI options over the config file, over the app defaults.
    pub fn from_cli(cli: &crate::Cli) -> Self {
        let file = FileConfig::load();
        Self {
            input: cli.input.clone().or(file.input),
            output: cli.output.clone().or(file.output),
            sample_rate: cli.sample_rate.or(file.sample_rate),
            preset: cli
                .preset
                .clone()
                .or(file.preset)
                .unwrap_or_else(|| "Clean".to_string()),
            record: cli.record.clone().or(file.record),
            pitch: cli.pitch.or(file.pitch).unwrap_or(0.0),
            formant: cli.formant.or(file.formant).unwrap_or(0.0),
            gate: cli.gate.or(file.gate).unwrap_or(-100.0),
            gain: cli.gain.or(file.gain).unwrap_or(1.0),
            run_seconds: cli.run_seconds,
            tone: cli.tone.or(file.tone).unwrap_or(0.0),
            virtual_mic: if cli.no_virtual_mic {
                false
            } else {
                file.virtual_mic
                    .unwrap_or(cfg!(any(target_os = "linux", target_os = "freebsd")))
            },
            pitch_correct: cli.pitch_correct.or(file.pitch_correct).unwrap_or(false),
        }
    }

    /// Resolve the preset name into an index, defaulting to "Clean" on error.
    fn preset_idx(&self) -> usize {
        Preset::find(&self.preset).unwrap_or(0)
    }

    pub fn to_control(&self) -> Control {
        Control {
            preset_idx: self.preset_idx(),
            pitch_delta: self.pitch,
            formant_delta: self.formant,
            gate_threshold_db: self.gate,
            gain: self.gain,
            record: self.record.is_some(),
            record_path: self.record.clone(),
            run_seconds: self.run_seconds,
            tone_hz: self.tone,
            pitch_correct: self.pitch_correct,
            ..Control::default()
        }
    }

    pub fn preset_names() -> String {
        presets()
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
