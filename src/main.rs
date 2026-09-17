use std::{
    io::{self, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use tui_big_text::{BigText, PixelSize};

/// Read at runtime, so editing prompt.txt needs no rebuild.
const PROMPT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/prompt.txt");
const QUOTE_EVERY_MINUTES: u64 = 2;

fn main() -> io::Result<()> {
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}

// ── speech ────────────────────────────────────────────────────────────────────

/// A single worker that speaks messages one after another, so a minute call
/// never talks over a quote.
fn spawn_speaker() -> Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        for text in rx {
            let _ = Command::new("say")
                .args(["-v", "Daniel"])
                .arg(&text)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    });
    tx
}

// ── ollama ────────────────────────────────────────────────────────────────────

fn model() -> String {
    std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:12b".into())
}

/// Make sure a server is up. If one is already running this exits at once
/// (port in use), so it's safe to call unconditionally.
fn ensure_ollama() {
    let _ = Command::new("ollama")
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Ask Ollama for a quote in the background; the reply lands on `tx`.
fn request_quote(tx: Sender<String>, previous: Option<String>) {
    thread::spawn(move || {
        let prompt = std::fs::read_to_string(PROMPT_PATH).unwrap_or_default();
        let mut prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            let _ = tx.send("prompt.txt is empty.".into());
            return;
        }
        if let Some(previous) = previous {
            prompt.push_str("\n\nPrevious intervention (do not repeat its wording or action):\n");
            prompt.push_str(&previous);
        }
        let mut child = match Command::new("ollama")
            .args(["run", "--think=false", &model()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => {
                let _ = tx.send("ollama is not available.".into());
                return;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
        }
        let out = child.wait_with_output().ok();
        let text = out
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "no response from the model.".into());
        let _ = tx.send(text);
    });
}

// ── app ───────────────────────────────────────────────────────────────────────

struct App {
    running: bool,
    started_at: Instant,
    minutes_spoken: u64,
    quote: Option<String>,
    pending: bool,
    sleep_preventer: Option<Child>,
    speaker: Sender<String>,
    quote_tx: Sender<String>,
    quote_rx: Receiver<String>,
}

impl App {
    fn new() -> Self {
        ensure_ollama();
        let (quote_tx, quote_rx) = mpsc::channel();
        let mut app = Self {
            running: false,
            started_at: Instant::now(),
            minutes_spoken: 0,
            quote: None,
            pending: false,
            sleep_preventer: None,
            speaker: spawn_speaker(),
            quote_tx,
            quote_rx,
        };
        app.start();
        app
    }

    fn start(&mut self) {
        self.sleep_preventer = Command::new("caffeinate")
            .args(["-i", "-w", &std::process::id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        self.running = true;
        self.started_at = Instant::now();
        self.minutes_spoken = 0;
        self.quote = None;
        self.fetch_quote();
    }

    fn stop(&mut self) {
        self.running = false;
        if let Some(mut child) = self.sleep_preventer.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn fetch_quote(&mut self) {
        self.pending = true;
        request_quote(self.quote_tx.clone(), self.quote.clone());
    }

    fn elapsed(&self) -> Duration {
        if self.running {
            self.started_at.elapsed()
        } else {
            Duration::ZERO
        }
    }

    fn tick(&mut self) {
        if let Ok(q) = self.quote_rx.try_recv() {
            self.quote = Some(q);
            self.pending = false;
        }
        if !self.running {
            return;
        }
        let minute = self.elapsed().as_secs() / 60;
        if minute > self.minutes_spoken {
            self.minutes_spoken = minute;
            let unit = if minute == 1 { "minute" } else { "minutes" };
            let _ = self.speaker.send(format!("{minute} {unit}"));
            if minute.is_multiple_of(QUOTE_EVERY_MINUTES) {
                if let Some(q) = &self.quote {
                    let _ = self.speaker.send(q.clone());
                }
                self.fetch_quote();
            }
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        loop {
            self.tick();
            terminal.draw(|f| self.draw(f))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Char('s') => {
                            if self.running {
                                self.stop()
                            } else {
                                self.start()
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // ── ui ────────────────────────────────────────────────────────────────

    fn draw(&self, f: &mut Frame) {
        let accent = if self.running {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let area = center(f.area(), 64, 20);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(accent))
            .padding(Padding::symmetric(2, 1))
            .title(Line::from(" focus ").fg(accent).bold())
            .title_alignment(Alignment::Center);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let [clock, status, quote, help] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        // clock
        let secs = self.elapsed().as_secs();
        let time = format!("{:02}:{:02}", secs / 60, secs % 60);
        let big = BigText::builder()
            .pixel_size(PixelSize::Full)
            .style(Style::new().fg(accent).bold())
            .lines(vec![time.into()])
            .alignment(Alignment::Center)
            .build();
        f.render_widget(big, clock);

        // status
        let (dot, label) = if self.running {
            ("●", "running")
        } else {
            ("○", "stopped")
        };
        let mut spans = vec![
            Span::raw(dot).fg(accent),
            Span::raw(" "),
            Span::raw(label).dim(),
        ];
        if self.pending {
            spans.push(Span::raw("  ·  thinking").dim().italic());
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
            status,
        );

        // quote
        let text = self.quote.as_deref().unwrap_or("");
        f.render_widget(
            Paragraph::new(text)
                .style(Style::new().add_modifier(Modifier::ITALIC))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            quote,
        );

        // help
        let toggle = if self.running { "stop" } else { "start" };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("space ").fg(accent),
                Span::raw(toggle).dim(),
                Span::raw("   q ").fg(accent),
                Span::raw("quit").dim(),
            ]))
            .alignment(Alignment::Center),
            help,
        );
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.stop();
    }
}

fn center(area: Rect, w: u16, h: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(w.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(h.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}
