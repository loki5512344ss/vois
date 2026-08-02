//! Full-screen interactive TUI built on ratatui: splash, dashboard with
//! presets / parameters / levels / stats, and a settings screen with arrow-key
//! and mouse control.

use crate::dsp::chain::{Control, Mode};
use crate::dsp::presets::PRESETS;
use crate::dsp::stats::Stats;
use crate::ui::SessionInfo;
use crossterm::event::{
    self, Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::{Frame, Terminal};
use std::io::{stdout, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LOGO: [&str; 7] = [
    "   ██╗   ██╗ ██████╗ ██╗███████╗   ",
    "   ██║   ██║██╔═══██╗██║██╔════╝   ",
    "   ██║   ██║██║   ██║██║███████╗   ",
    "   ╚██╗ ██╔╝██║   ██║██║╚════██║   ",
    "    ╚████╔╝ ╚██████╔╝██║███████║   ",
    "     ╚═══╝   ╚═════╝ ╚═╝╚══════╝   ",
    "        real-time voice changer     ",
];

const SETTING_INFO: [&str; 8] = [
    "voice character, previewed live",
    "pitch: higher = girl/kid, lower = deep",
    "formant: brightness / timbre of the voice",
    "noise gate: audio below this dB is cut",
    "output loudness, in dB",
    "silence the output",
    "LIVE = processed, PASSTHROUGH = raw input (A/B)",
    "write processed audio to a WAV file",
];

const HELP_TEXT: &str = "\
  KEY / MOUSE                 ACTION
  ───────────────────────────────────────────────
  ↑ ↓  (or mouse)             select preset / settings row
  ← →                         change value
  Enter                       toggle / apply
  S  /  Tab                   toggle SETTINGS screen
  Esc                         back to main screen
  H                           toggle this help screen
  Q  /  Ctrl+C                quit
  0 - 9                       jump to preset
  M                           mute      R  record
  B                           A/B passthrough mode
";

const METER_FLOOR_DB: f32 = -60.0;

fn db(rms: f32) -> f32 {
    if rms <= 0.0 {
        METER_FLOOR_DB
    } else {
        (20.0 * rms.log10()).clamp(METER_FLOOR_DB, 6.0)
    }
}

fn meter_ratio(db: f32) -> f64 {
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0) as f64
}

fn meter_style(level_db: f32) -> Style {
    if level_db >= -6.0 {
        Style::default().fg(Color::Red)
    } else if level_db >= -20.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Main,
    Settings,
    Help,
}

struct App {
    control: Arc<Mutex<Control>>,
    stats: Arc<Mutex<Stats>>,
    info: SessionInfo,
    view: View,
    settings_cursor: usize,
    splash_until: Instant,
    started: Instant,
    running: bool,
}

impl App {
    /// (preset_idx, preset_name, pitch_st, formant_ratio, gate_db, gain_lin,
    ///  mute, record, tone, mode)
    fn snapshot(&self) -> (usize, String, f32, f32, f32, f32, bool, bool, bool, Mode) {
        self.control
            .lock()
            .map(|c| {
                let preset = &PRESETS[c.preset_idx.min(PRESETS.len() - 1)];
                (
                    c.preset_idx.min(PRESETS.len() - 1),
                    preset.name.to_string(),
                    preset.pitch_semitones + c.pitch_delta,
                    preset.formant_ratio * 2f32.powf(c.formant_delta / 12.0),
                    c.gate_threshold_db,
                    c.gain,
                    c.mute,
                    c.record,
                    c.tone_hz > 0.0,
                    c.mode,
                )
            })
            .unwrap_or((
                0,
                "Clean".into(),
                0.0,
                1.0,
                -100.0,
                1.0,
                false,
                false,
                false,
                Mode::Live,
            ))
    }

    fn setting_value(&self, idx: usize) -> String {
        let (_, _name, pitch, formant, gate, gain, mute, record, _tone, mode) = self.snapshot();
        match idx {
            0 => PRESETS[self
                .control
                .lock()
                .map(|c| c.preset_idx.min(PRESETS.len() - 1))
                .unwrap_or(0)]
            .name
            .to_string(),
            1 => format!("{pitch:+.1} st"),
            2 => format!("{formant:.2}×"),
            3 => {
                if gate <= -90.0 {
                    "off".to_string()
                } else {
                    format!("{gate:.0} dB")
                }
            }
            4 => format!("{:+.1} dB", 20.0 * gain.max(1e-6).log10()),
            5 => if mute { "ON" } else { "off" }.to_string(),
            6 => match mode {
                Mode::Live => "LIVE".to_string(),
                Mode::Passthrough => "PASSTHROUGH".to_string(),
            },
            7 => if record { "recording" } else { "off" }.to_string(),
            _ => String::new(),
        }
    }

    /// Adjust a settings row. `step` is negative/positive; toggles ignore it.
    fn adjust(&self, idx: usize, step: i32) {
        self.with_ctrl(|c| match idx {
            0 => {
                let n = PRESETS.len() as i32;
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
            _ => {}
        });
    }

    fn on_key(&mut self, key: crossterm::event::KeyEvent) {
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
            KeyCode::Up => self
                .with_ctrl(|c| c.preset_idx = (c.preset_idx + PRESETS.len() - 1) % PRESETS.len()),
            KeyCode::Down => self.with_ctrl(|c| c.preset_idx = (c.preset_idx + 1) % PRESETS.len()),
            KeyCode::Left => self.adjust(1, -1),
            KeyCode::Right => self.adjust(1, 1),
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Tab => self.view = View::Settings,
            KeyCode::Char('h') | KeyCode::Char('H') => self.view = View::Help,
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.with_ctrl(|c| c.preset_idx = (c.preset_idx + 1) % PRESETS.len())
            }
            KeyCode::Char(d) if d.is_ascii_digit() => self.with_ctrl(|c| {
                let n = d.to_digit(10).unwrap() as usize;
                c.preset_idx = if n == 0 {
                    PRESETS.len() - 1
                } else {
                    (n - 1).min(PRESETS.len() - 1)
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

    fn on_mouse(&mut self, mouse: MouseEvent) {
        if Instant::now() < self.splash_until {
            self.splash_until = Instant::now();
            return;
        }
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let area = self.terminal_size();
            match self.view {
                View::Main => {
                    let rects = main_layout(area);
                    let presets_rect = rects.0;
                    if mouse.row >= presets_rect.y
                        && mouse.row < presets_rect.y + presets_rect.height
                    {
                        let idx = (mouse.row - presets_rect.y) as usize;
                        if idx < PRESETS.len() {
                            self.with_ctrl(|c| c.preset_idx = idx);
                        }
                    }
                }
                View::Settings => {
                    let rects = settings_layout(area);
                    let list_rect = rects.0;
                    if mouse.row >= list_rect.y && mouse.row < list_rect.y + list_rect.height {
                        let idx = (mouse.row - list_rect.y) as usize;
                        if idx < 8 {
                            self.settings_cursor = idx;
                        }
                    }
                }
                View::Help => self.view = View::Main,
            }
        }
    }

    fn terminal_size(&self) -> Rect {
        crossterm::terminal::size()
            .map(|(w, h)| Rect::new(0, 0, w, h))
            .unwrap_or(Rect::new(0, 0, 100, 30))
    }

    fn set_quit(&mut self) {
        self.with_ctrl(|c| c.quit = true);
        self.running = false;
    }

    fn with_ctrl(&self, f: impl FnOnce(&mut Control)) {
        if let Ok(mut c) = self.control.lock() {
            f(&mut c);
        }
    }

    fn render(&mut self, f: &mut Frame) {
        if Instant::now() < self.splash_until {
            self.render_splash(f);
            return;
        }
        match self.view {
            View::Main => self.render_main(f),
            View::Settings => self.render_settings(f),
            View::Help => self.render_help(f),
        }
    }

    fn render_splash(&self, f: &mut Frame) {
        let area = f.area();
        let mut lines: Vec<Line> = Vec::new();
        for (i, l) in LOGO.iter().enumerate() {
            let color = match i {
                0 | 1 => Color::Cyan,
                2 | 3 => Color::Magenta,
                4 | 5 => Color::Blue,
                _ => Color::Gray,
            };
            lines.push(Line::styled(
                l.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "  press any key, click, or use the arrow keys ...",
        ));
        lines.push(Line::styled(
            "  audio is already running",
            Style::default().fg(Color::DarkGray),
        ));
        let para = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
        // Center in the full area, but never taller than the terminal.
        let rect = centered_rect(70, 55, area);
        f.render_widget(para, rect);
    }

    fn render_help(&self, f: &mut Frame) {
        let para = Paragraph::new(HELP_TEXT)
            .style(Style::default().fg(Color::White))
            .block(block(" vois · help "));
        f.render_widget(para, f.area());
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let (_, _name, _p, _f, _g, _gain, mute, record, tone, mode) = self.snapshot();
        let mode_txt = match mode {
            Mode::Live => "LIVE",
            Mode::Passthrough => "A/B PASSTHROUGH",
        };
        let rec_txt = if record { "● REC" } else { "" };
        let tone_txt = if tone { "● TONE" } else { "" };
        let mute_txt = if mute { "MUTED" } else { "" };
        let header = Line::from(vec![
            Span::styled(
                "  VOIS ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "real-time voice changer",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::raw("   ·   "),
            Span::styled(
                mode_txt,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                rec_txt,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(tone_txt, Style::default().fg(Color::Blue)),
            Span::raw("  "),
            Span::styled(
                mute_txt,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(header), area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect, hint: &str) {
        let footer = Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray)));
        f.render_widget(Paragraph::new(footer), area);
    }

    fn render_main(&mut self, f: &mut Frame) {
        let area = f.area();
        let rects = main_layout(area);
        let (head, body, foot) = (rects.0, rects.1, rects.2);

        self.render_header(f, head);

        let (preset_idx, _name, pitch, formant, gate, gain, mute, _record, _tone, _mode) =
            self.snapshot();

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(38),
                Constraint::Percentage(28),
                Constraint::Percentage(34),
            ])
            .split(body);

        // Presets (navigable, live preview).
        let mut plines = Vec::new();
        for (i, p) in PRESETS.iter().enumerate() {
            let selected = i == preset_idx;
            let label = format!(
                "{}{}  {}",
                if selected { "▶" } else { " " },
                if selected { "" } else { " " },
                p.name
            );
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            plines.push(Line::styled(label, style));
        }
        let para = Paragraph::new(plines).block(block(" presets · ↑↓ to switch "));
        f.render_widget(para, cols[0]);

        // Parameters + info.
        let gate_txt = if gate <= -90.0 {
            "off".to_string()
        } else {
            format!("{gate:.0} dB")
        };
        let gain_db = 20.0 * gain.max(1e-6).log10();
        let param_lines = vec![
            p_row("preset", &_name, Color::Yellow, "voice character"),
            p_row(
                "pitch",
                &format!("{pitch:+.1} st"),
                Color::Cyan,
                "higher=girl, lower=deep",
            ),
            p_row(
                "formant",
                &format!("{formant:.2}×"),
                Color::Magenta,
                "brightness/timbre",
            ),
            p_row("gate", &gate_txt, Color::Blue, "noise below this dB is cut"),
            p_row(
                "gain",
                &format!("{gain_db:+.1} dB"),
                Color::Green,
                "output loudness",
            ),
            p_row(
                "mute",
                if mute { "ON" } else { "off" },
                if mute { Color::Red } else { Color::DarkGray },
                "silence output",
            ),
        ];
        let para = Paragraph::new(param_lines).block(block(" parameters · info "));
        f.render_widget(para, cols[1]);

        self.render_stats(f, cols[2]);

        self.render_footer(
            f,
            foot,
            " ↑↓ preset | ←→ pitch | S settings | H help | Q quit ",
        );
    }

    fn render_settings(&mut self, f: &mut Frame) {
        let area = f.area();
        let rects = settings_layout(area);
        let (head, list_rect, foot) = (rects.0, rects.1, rects.2);

        self.render_header(f, head);

        let labels = [
            "Preset",
            "Pitch",
            "Formant",
            "Gate (min)",
            "Gain",
            "Mute",
            "Mode",
            "Record",
        ];
        let mut lines = Vec::new();
        for (i, label) in labels.iter().enumerate() {
            let value = self.setting_value(i);
            let selected = i == self.settings_cursor;
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let info_style = Style::default().fg(Color::DarkGray);
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{}{} {:<12}",
                        if selected { "▶" } else { " " },
                        if selected { "" } else { " " },
                        label
                    ),
                    style,
                ),
                Span::styled(format!("{:<16}", value), style),
                Span::styled(SETTING_INFO[i], info_style),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  use ↑ ↓ to select, ← → (or Enter) to change, Esc to go back",
            Style::default().fg(Color::DarkGray),
        ));
        let para = Paragraph::new(lines).block(block(" settings "));
        f.render_widget(para, list_rect);

        self.render_footer(
            f,
            foot,
            " ↑↓ select | ←→ change | Esc back | H help | Q quit ",
        );
    }

    fn render_stats(&self, f: &mut Frame, area: Rect) {
        let s = self
            .stats
            .lock()
            .map(|s| {
                (
                    s.input_rms,
                    s.output_rms,
                    s.peak_in,
                    s.peak_out,
                    s.blocks,
                    s.samples,
                    s.cpu_pct,
                    s.dsp_latency_ms,
                    s.started,
                )
            })
            .unwrap_or((0.0, 0.0, 0.0, 0.0, 0, 0, 0.0, 0.0, Instant::now()));
        let (in_rms, out_rms, peak_in, peak_out, blocks, samples, cpu, latency, started) = s;

        let in_db = db(in_rms);
        let out_db = db(out_rms);
        let peak_in_db = db(peak_in);
        let peak_out_db = db(peak_out);

        let uptime = started.elapsed().as_secs();
        let up = format!(
            "{:02}:{:02}:{:02}",
            uptime / 3600,
            (uptime / 60) % 60,
            uptime % 60
        );
        let secs = started.elapsed().as_secs_f32().max(0.1);
        let hz = samples as f32 / secs;

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  peak in   ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{peak_in_db:+.0} dB"), meter_style(peak_in_db)),
                Span::styled("   peak out ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{peak_out_db:+.0} dB"), meter_style(peak_out_db)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  dsp load  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{cpu:.1} %"),
                    if cpu > 50.0 {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Green)
                    },
                ),
                Span::raw("  "),
                Span::styled("latency", Style::default().fg(Color::DarkGray)),
                Span::raw(format!(" {latency:.0} ms")),
            ]),
            Line::from(vec![
                Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!(
                    "{} Hz  {} ch → {} ch  ({} → {})",
                    self.info.sample_rate,
                    self.info.in_ch,
                    self.info.out_ch,
                    self.info.format_in,
                    self.info.format_out
                )),
            ]),
            Line::from(vec![
                Span::styled("  uptime    ", Style::default().fg(Color::DarkGray)),
                Span::raw(up),
            ]),
            Line::from(vec![
                Span::styled("  blocks    ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{blocks}  ({hz:.0} blk/s)")),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!("  in:  {}", self.info.input),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                format!("  out: {}", self.info.output),
                Style::default().fg(Color::DarkGray),
            )),
        ];

        // Meters on top of the column.
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(area);

        let in_gauge = Gauge::default()
            .ratio(meter_ratio(in_db))
            .gauge_style(meter_style(in_db));
        let out_gauge = Gauge::default()
            .ratio(meter_ratio(out_db))
            .gauge_style(meter_style(out_db));

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  in  "),
                Span::styled(format!("{in_db:+.0} dB"), meter_style(in_db)),
            ])),
            vert[0],
        );
        f.render_widget(in_gauge, vert[1]);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  out "),
                Span::styled(format!("{out_db:+.0} dB"), meter_style(out_db)),
            ])),
            vert[2],
        );
        f.render_widget(out_gauge, vert[3]);
        f.render_widget(Paragraph::new(lines), vert[4]);
    }
}

/// Layout rects for the main screen: (header, body, footer).
fn main_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2])
}

/// Layout rects for the settings screen: (header, list, footer).
fn settings_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2])
}

fn p_row(label: &str, value: &str, color: Color, info: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {label:<8}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            value.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   ({info})"), Style::default().fg(Color::DarkGray)),
    ])
}

fn block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::DarkGray))
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let height = ((area.height as u32 * percent_y as u32 / 100) as u16).max(12);
    let top = area.y + (area.height.saturating_sub(height) / 2);
    let rect = Rect::new(area.x, top, area.width, height);
    let width = ((rect.width as u32 * percent_x as u32 / 100) as u16).max(30);
    let x = rect.x + (rect.width.saturating_sub(width) / 2);
    Rect::new(x, rect.y, width, rect.height)
}

pub fn run(control: Arc<Mutex<Control>>, stats: Arc<Mutex<Stats>>, info: SessionInfo) {
    let run_seconds = control.lock().ok().and_then(|c| c.run_seconds).unwrap_or(0);

    if !std::io::stdin().is_terminal() {
        let started = Instant::now();
        loop {
            let done = control
                .lock()
                .map(|c| {
                    c.quit || run_seconds > 0 && started.elapsed().as_secs() >= run_seconds as u64
                })
                .unwrap_or(true);
            if done {
                if let Ok(mut c) = control.lock() {
                    c.quit = true;
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        return;
    }

    enable_raw_mode().expect("failed to enable raw mode");
    let _ = crossterm::execute!(stdout(), crossterm::event::EnableMouseCapture);
    let mut terminal = match Terminal::new(CrosstermBackend::new(stdout())) {
        Ok(t) => t,
        Err(e) => {
            disable_raw_mode().ok();
            eprintln!("tui init failed: {e}");
            return;
        }
    };
    let _ = terminal.clear();

    let mut app = App {
        control,
        stats,
        info,
        view: View::Main,
        settings_cursor: 0,
        splash_until: Instant::now() + Duration::from_millis(1500),
        started: Instant::now(),
        running: true,
    };

    while app.running {
        let _ = terminal.draw(|f| app.render(f));
        let _ = stdout().flush();

        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(ev) = event::read() {
                match ev {
                    Event::Key(key) => app.on_key(key),
                    Event::Mouse(m) => app.on_mouse(m),
                    _ => {}
                }
            }
        }

        if run_seconds > 0 && app.started.elapsed().as_secs() >= run_seconds as u64 {
            app.set_quit();
        }
    }

    let _ = terminal.show_cursor();
    let _ = crossterm::execute!(stdout(), crossterm::event::DisableMouseCapture);
    disable_raw_mode().ok();
    println!("shutting down...");
}
