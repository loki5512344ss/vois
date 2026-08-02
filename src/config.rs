use crate::dsp::chain::Control;
use crate::dsp::presets::PRESETS;

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
}

impl AppConfig {
    pub fn from_cli(cli: &crate::Cli) -> Self {
        Self {
            input: cli.input.clone(),
            output: cli.output.clone(),
            sample_rate: cli.sample_rate,
            preset: cli.preset.clone(),
            record: cli.record.clone(),
            pitch: cli.pitch,
            formant: cli.formant,
            gate: cli.gate,
            gain: cli.gain,
            run_seconds: cli.run_seconds,
            tone: cli.tone,
            virtual_mic: !cli.no_virtual_mic,
        }
    }

    /// Resolve the preset name into an index, defaulting to "Clean" on error.
    fn preset_idx(&self) -> usize {
        crate::dsp::presets::Preset::find(&self.preset).unwrap_or(0)
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
            ..Control::default()
        }
    }

    pub fn preset_names() -> String {
        PRESETS
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
            .join(", ")
    }
}
