//! Frame rendering for each screen, mouse input and the main TUI run loop.

use crate::dsp::control::Control;
use crate::dsp::control::Stats;
use crate::dsp::preset::presets;
use crate::ui::app::{App, View};
use crate::ui::screens::{
    header_line, help, params_paragraph, presets_paragraph, settings_paragraph, splash,
    stats_column, StatsSnap,
};
use crate::ui::widgets::{self, main_layout, settings_layout};
use crate::ui::SessionInfo;
use crossterm::event::{self, Event, MouseButton, MouseEvent, MouseEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use std::io::{stdout, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

impl App {
    fn render(&mut self, f: &mut Frame) {
        if Instant::now() < self.splash_until {
            splash(f, f.area());
            return;
        }
        match self.view {
            View::Main => self.render_main(f),
            View::Settings => self.render_settings(f),
            View::Help => help(f, f.area()),
        }
    }

    fn render_main(&mut self, f: &mut Frame) {
        let area = f.area();
        let (head, body, foot) = main_layout(area);
        self.render_header(f, head);

        let s = self.snapshot();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(38),
                Constraint::Percentage(28),
                Constraint::Percentage(34),
            ])
            .split(body);

        f.render_widget(presets_paragraph(s.preset_idx), cols[0]);
        f.render_widget(params_paragraph(&s), cols[1]);
        self.render_stats(f, cols[2]);

        self.render_footer(
            f,
            foot,
            " ↑↓ preset | ←→ pitch | S settings | H help | Q quit ",
        );
    }

    fn render_settings(&mut self, f: &mut Frame) {
        let area = f.area();
        let (head, list_rect, foot) = settings_layout(area);
        self.render_header(f, head);

        let values: Vec<String> = (0..9).map(|i| self.setting_value(i)).collect();
        f.render_widget(settings_paragraph(&values, self.settings_cursor), list_rect);

        self.render_footer(
            f,
            foot,
            " ↑↓ select | ←→ change | Esc back | H help | Q quit ",
        );
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Paragraph::new(header_line(&self.snapshot())), area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect, hint: &str) {
        let footer = Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray)));
        f.render_widget(Paragraph::new(footer), area);
    }

    fn render_stats(&self, f: &mut Frame, area: Rect) {
        let snap = self
            .stats
            .lock()
            .map(|s| StatsSnap {
                in_db: widgets::db(s.input_rms),
                out_db: widgets::db(s.output_rms),
                peak_in: widgets::db(s.peak_in),
                peak_out: widgets::db(s.peak_out),
                cpu: s.cpu_pct,
                latency: s.dsp_latency_ms,
                blocks: s.blocks,
                samples: s.samples,
                uptime: s.started.elapsed().as_secs(),
            })
            .unwrap_or_default();
        stats_column(f, area, &self.info, &snap);
    }

    /// Map a mouse click onto the current screen (preset / settings row).
    fn on_mouse(&mut self, mouse: MouseEvent) {
        if Instant::now() < self.splash_until {
            self.splash_until = Instant::now();
            return;
        }
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let area = crossterm::terminal::size()
                .map(|(w, h)| Rect::new(0, 0, w, h))
                .unwrap_or(Rect::new(0, 0, 100, 30));
            match self.view {
                View::Main => {
                    let presets_rect = main_layout(area).0;
                    if mouse.row >= presets_rect.y
                        && mouse.row < presets_rect.y + presets_rect.height
                    {
                        let idx = (mouse.row - presets_rect.y) as usize;
                        if idx < presets().len() {
                            self.with_ctrl(|c| c.preset_idx = idx);
                        }
                    }
                }
                View::Settings => {
                    let list_rect = settings_layout(area).0;
                    if mouse.row >= list_rect.y && mouse.row < list_rect.y + list_rect.height {
                        let idx = (mouse.row - list_rect.y) as usize;
                        if idx < 9 {
                            self.settings_cursor = idx;
                        }
                    }
                }
                View::Help => self.view = View::Main,
            }
        }
    }
}

/// Run headless until `run_seconds` elapse or `quit` is set.
fn headless_wait(control: &Arc<Mutex<Control>>, run_seconds: u32) {
    let started = Instant::now();
    loop {
        let done = control
            .lock()
            .map(|c| c.quit || run_seconds > 0 && started.elapsed().as_secs() >= run_seconds as u64)
            .unwrap_or(true);
        if done {
            if let Ok(mut c) = control.lock() {
                c.quit = true;
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn run(control: Arc<Mutex<Control>>, stats: Arc<Mutex<Stats>>, info: SessionInfo) {
    let run_seconds = control.lock().ok().and_then(|c| c.run_seconds).unwrap_or(0);

    if !std::io::stdin().is_terminal() || enable_raw_mode().is_err() {
        // No real terminal: keep the audio running headless.
        headless_wait(&control, run_seconds);
        return;
    }
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
