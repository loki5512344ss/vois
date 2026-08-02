//! App state, the live snapshot and keyboard / mouse handling.

use crate::dsp::control::Stats;
use crate::dsp::control::{Control, Mode};
use crate::dsp::preset::presets;
use crate::ui::SessionInfo;
use crossterm::event::{KeyCode, KeyModifiers};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum View {
    Main,
    Settings,
    Help,
}

pub(super) struct App {
    pub(super) control: Arc<Mutex<Control>>,
    pub(super) stats: Arc<Mutex<Stats>>,
    pub(super) info: SessionInfo,
    pub(super) view: View,
    pub(super) settings_cursor: usize,
    pub(super) splash_until: Instant,
    pub(super) started: Instant,
    pub(super) running: bool,
}

/// Copy of the live control values used by the render layer.
pub(super) struct Snap {
    pub(super) preset_idx: usize,
    pub(super) name: String,
    pub(super) pitch: f32,
    pub(super) formant: f32,
    pub(super) gate: f32,
    pub(super) gain: f32,
    pub(super) mute: bool,
    pub(super) record: bool,
    pub(super) tone: bool,
    pub(super) tune: bool,
    pub(super) mode: Mode,
}

impl App {
    /// Current live values as a plain snapshot for rendering.
    pub(super) fn snapshot(&self) -> Snap {
        self.control
            .lock()
            .map(|c| {
                let p = &presets()[c.preset_idx.min(presets().len() - 1)];
                Snap {
                    preset_idx: c.preset_idx.min(presets().len() - 1),
                    name: p.name.to_string(),
                    pitch: p.pitch_semitones + c.pitch_delta,
                    formant: p.formant_ratio * 2f32.powf(c.formant_delta / 12.0),
                    gate: if c.gate_threshold_db > -90.0 {
                        c.gate_threshold_db
                    } else {
                        p.gate_db
                    },
                    gain: c.gain,
                    mute: c.mute,
                    record: c.record,
                    tone: c.tone_hz > 0.0,
                    tune: c.pitch_correct,
                    mode: c.mode,
                }
            })
            .unwrap_or(Snap {
                preset_idx: 0,
                name: "Clean".into(),
                pitch: 0.0,
                formant: 1.0,
                gate: -100.0,
                gain: 1.0,
                mute: false,
                record: false,
                tone: false,
                tune: false,
                mode: Mode::Live,
            })
    }

    /// Rendered value of a settings row.
    pub(super) fn setting_value(&self, idx: usize) -> String {
        let s = self.snapshot();
        match idx {
            0 => s.name,
            1 => format!("{:+.1} st", s.pitch),
            2 => format!("{:.2}×", s.formant),
            3 => {
                if s.gate <= -90.0 {
                    "off".to_string()
                } else {
                    format!("{:.0} dB", s.gate)
                }
            }
            4 => format!("{:+.1} dB", 20.0 * s.gain.max(1e-6).log10()),
            5 => if s.mute { "ON" } else { "off" }.to_string(),
            6 => match s.mode {
                Mode::Live => "LIVE".to_string(),
                Mode::Passthrough => "PASSTHROUGH".to_string(),
            },
            7 => if s.record { "recording" } else { "off" }.to_string(),
            8 => if s.tune { "ON" } else { "off" }.to_string(),
            _ => String::new(),
        }
    }

    /// Adjust a settings row. `step` is negative/positive; toggles ignore it.
    fn adjust(&self, idx: usize, step: i32) {
        self.with_ctrl(|c| match idx {
            0 => {
                let n = presets().len() as i32;
                c.preset_idx = ((c.preset_idx as i32 + step).rem_euclid(n)) as usize;
            }
            1 => c.pitch_delta += step as f32,
            2 => c.formant_delta += step as f32,
            3 => {
                c.gate_threshold_db =
                    (c.gate_threshold_db + step as f32 * 2.0).clamp(-100.0, -20.0);
            }
            4 => {
                let db = 20.0 * c.gain.max(1e-6).log10() + step as f32;
                c.gain = 10f32.powf(db / 20.0).clamp(0.0316, 4.0); // -30 .. +12 dB
            }
            5 => c.mute = !c.mute,
            6 => c.mode.toggle(),
            7 => c.record = !c.record,
            8 => c.pitch_correct = !c.pitch_correct,
            _ => {}
        });
    }

    pub(super) fn on_key(&mut self, key: crossterm::event::KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let quit = key.code == KeyCode::Char('q')
            || (ctrl && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q')));

        // Any key dismisses the splash.
        if Instant::now() < self.splash_until {
            self.splash_until = Instant::now();
            if quit {
                self.set_quit();
            }
            return;
        }

        if quit {
            self.set_quit();
            return;
        }

        match self.view {
            View::Help => {
                if key.code == KeyCode::Esc
                    || key.code == KeyCode::Char('h')
                    || key.code == KeyCode::Char('H')
                {
                    self.view = View::Main;
                }
            }
            View::Main => self.on_key_main(key),
            View::Settings => self.on_key_settings(key),
        }
    }

    fn on_key_main(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up => self.with_ctrl(|c| {
                c.preset_idx = (c.preset_idx + presets().len() - 1) % presets().len()
            }),
            KeyCode::Down => {
                self.with_ctrl(|c| c.preset_idx = (c.preset_idx + 1) % presets().len())
            }
            KeyCode::Left => self.adjust(1, -1),
            KeyCode::Right => self.adjust(1, 1),
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Tab => self.view = View::Settings,
            KeyCode::Char('h') | KeyCode::Char('H') => self.view = View::Help,
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.with_ctrl(|c| c.preset_idx = (c.preset_idx + 1) % presets().len())
            }
            KeyCode::Char(d) if d.is_ascii_digit() => self.with_ctrl(|c| {
                let n = d.to_digit(10).unwrap() as usize;
                c.preset_idx = if n == 0 {
                    presets().len() - 1
                } else {
                    (n - 1).min(presets().len() - 1)
                };
            }),
            KeyCode::Char('[') => self.adjust(1, -1),
            KeyCode::Char(']') => self.adjust(1, 1),
            KeyCode::Char('{') => self.adjust(2, -1),
            KeyCode::Char('}') => self.adjust(2, 1),
            KeyCode::Char('g') | KeyCode::Char('G') => self.adjust(3, -1),
            KeyCode::Char('b') | KeyCode::Char('B') => self.adjust(6, 1),
            KeyCode::Char('m') | KeyCode::Char('M') => self.adjust(5, 1),
            KeyCode::Char('r') | KeyCode::Char('R') => self.adjust(7, 1),
            KeyCode::Char('+') | KeyCode::Char('=') => self.adjust(4, 1),
            KeyCode::Char('-') => self.adjust(4, -1),
            _ => {}
        }
    }

    fn on_key_settings(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up => self.settings_cursor = self.settings_cursor.saturating_sub(1),
            KeyCode::Down => self.settings_cursor = (self.settings_cursor + 1).min(7),
            KeyCode::Left => self.adjust(self.settings_cursor, -1),
            KeyCode::Right => self.adjust(self.settings_cursor, 1),
            KeyCode::Enter => self.adjust(self.settings_cursor, 1),
            KeyCode::Esc | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Tab => {
                self.view = View::Main
            }
            KeyCode::Char('h') | KeyCode::Char('H') => self.view = View::Help,
            KeyCode::Char('q') => {}
            _ => {}
        }
    }

    pub(super) fn set_quit(&mut self) {
        self.with_ctrl(|c| c.quit = true);
        self.running = false;
    }

    pub(super) fn with_ctrl(&self, f: impl FnOnce(&mut Control)) {
        if let Ok(mut c) = self.control.lock() {
            f(&mut c);
        }
    }
}
