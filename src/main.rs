use std::{
    io::{self, Read, Write},
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

/// Pause Music / Spotify and the system Now Playing session while we speak,
/// then resume only what we paused. Chrome tabs are not scanned.
/// Owns the speech thread so Drop can join it and un-duck audio before exit.
struct Speaker {
    tx: Option<Sender<String>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Speaker {
    fn send(&self, text: String) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(text);
        }
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn spawn_speaker() -> Speaker {
    let (tx, rx) = mpsc::channel::<String>();
    let thread = thread::spawn(move || {
        while let Ok(text) = rx.recv() {
            // Hold the pause across back-to-back lines (minute + quote) so
            // other audio does not flicker between them.
            let _guard = DuckGuard(Some(duck_other_audio()));
            speak(&text);
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(next) => speak(&next),
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    });
    Speaker {
        tx: Some(tx),
        thread: Some(thread),
    }
}

fn speak(text: &str) {
    let _ = Command::new("say")
        .args(["-v", "Daniel"])
        .arg(text)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

struct Ducked {
    music: bool,
    spotify: bool,
    now_playing: Option<String>,
}

struct DuckGuard(Option<Ducked>);

impl Drop for DuckGuard {
    fn drop(&mut self) {
        if let Some(ducked) = self.0.take() {
            unduck_other_audio(ducked);
        }
    }
}

fn duck_other_audio() -> Ducked {
    Ducked {
        music: pause_app("Music"),
        spotify: pause_app("Spotify"),
        now_playing: run_osascript(true, NOW_PLAYING_PAUSE, &[])
            .as_deref()
            .and_then(parse_now_playing),
    }
}

fn unduck_other_audio(ducked: Ducked) {
    if let Some(client) = ducked.now_playing.as_deref() {
        let _ = run_osascript(true, NOW_PLAYING_RESUME, &[client]);
    }
    if ducked.spotify {
        resume_app("Spotify");
    }
    if ducked.music {
        resume_app("Music");
    }
}

fn pause_app(name: &str) -> bool {
    process_running(name) && run_osascript(false, APP_PAUSE, &[name]).as_deref() == Some("paused")
}

fn resume_app(name: &str) {
    if process_running(name) {
        let _ = run_osascript(false, APP_RESUME, &[name]);
    }
}

fn process_running(name: &str) -> bool {
    // AppleScript's `is running` can be true for Music's background agent and
    // then `tell application` launches the real app. pgrep checks the process.
    Command::new("pgrep")
        .args(["-x", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn parse_now_playing(s: &str) -> Option<String> {
    let id = s.trim().strip_prefix("paused:")?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn run_osascript(javascript: bool, script: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("osascript");
    if javascript {
        cmd.args(["-l", "JavaScript"]);
    }
    cmd.arg("-").args(args);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes());
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut buf = Vec::new();
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = stdout.read_to_end(&mut buf);
                }
                if !status.success() {
                    return None;
                }
                let text = String::from_utf8_lossy(&buf).trim().to_string();
                return (!text.is_empty()).then_some(text);
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(15)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

const APP_PAUSE: &str = r#"
on run argv
	set appName to item 1 of argv
	try
		return run script "tell application \"" & appName & "\"
			try
				with timeout of 2 seconds
					if player state is playing then
						pause
						return \"paused\"
					end if
				end timeout
			end try
			return \"idle\"
		end tell"
	end try
	return "idle"
end run
"#;

const APP_RESUME: &str = r#"
on run argv
	set appName to item 1 of argv
	try
		run script "tell application \"" & appName & "\"
			try
				with timeout of 2 seconds
					if player state is paused then play
				end timeout
			end try
		end tell"
	end try
end run
"#;

// MediaRemote is how Control Center / media keys talk to the current player,
// including YouTube in a browser. Tabs are never enumerated.
const NOW_PLAYING_PAUSE: &str = r#"
function run() {
  const MediaRemote = $.NSBundle.bundleWithPath("/System/Library/PrivateFrameworks/MediaRemote.framework/");
  MediaRemote.load;
  ObjC.bindFunction("MRMediaRemoteSendCommand", ["bool", ["int", "id"]]);
  const R = $.NSClassFromString("MRNowPlayingRequest");
  if (!R) return "idle";
  function playing() {
    try {
      const u = ObjC.unwrap(R.localIsPlaying);
      return u === true || u === 1;
    } catch (e) {
      return false;
    }
  }
  function client() {
    try {
      return R.localNowPlayingPlayerPath.client.bundleIdentifier.js || "";
    } catch (e) {
      return "";
    }
  }
  if (!playing()) return "idle";
  const who = client();
  if (!who) return "idle";
  if (!$.MRMediaRemoteSendCommand(1, ObjC.nil)) return "idle";
  return "paused:" + who;
}
"#;

const NOW_PLAYING_RESUME: &str = r#"
function run(argv) {
  const expected = argv[0] || "";
  const MediaRemote = $.NSBundle.bundleWithPath("/System/Library/PrivateFrameworks/MediaRemote.framework/");
  MediaRemote.load;
  ObjC.bindFunction("MRMediaRemoteSendCommand", ["bool", ["int", "id"]]);
  const R = $.NSClassFromString("MRNowPlayingRequest");
  if (!R || !expected) return "idle";
  function playing() {
    try {
      const u = ObjC.unwrap(R.localIsPlaying);
      return u === true || u === 1;
    } catch (e) {
      return false;
    }
  }
  function client() {
    try {
      return R.localNowPlayingPlayerPath.client.bundleIdentifier.js || "";
    } catch (e) {
      return "";
    }
  }
  if (playing()) return "already-playing";
  const who = client();
  if (who && expected && who !== expected) return "client-changed";
  $.MRMediaRemoteSendCommand(0, ObjC.nil);
  return "resumed";
}
"#;

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
    speaker: Speaker,
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
            self.speaker.send(format!("{minute} {unit}"));
            if minute.is_multiple_of(QUOTE_EVERY_MINUTES) {
                if let Some(q) = &self.quote {
                    self.speaker.send(q.clone());
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
            .title(Line::from(" presence ").fg(accent).bold())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_now_playing_pause() {
        assert_eq!(parse_now_playing("idle"), None);
        assert_eq!(parse_now_playing("paused:"), None);
        assert_eq!(
            parse_now_playing("paused:com.google.Chrome"),
            Some("com.google.Chrome".into())
        );
    }

    #[test]
    fn pause_skips_closed_apps() {
        assert!(!process_running("ThisAppDoesNotExistAtAll123"));
        assert!(!pause_app("ThisAppDoesNotExistAtAll123"));
    }

    #[test]
    fn speaker_joins_when_dropped() {
        drop(spawn_speaker());
    }
}
