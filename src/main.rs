use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
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

/// Checkout copies, used when Application Support has no editable file yet.
const PROMPT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/prompt.txt");
const QUOTES_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/quotes.csv");
const BUNDLED_PROMPT: &str = include_str!("../prompt.txt");
const BUNDLED_QUOTES: &str = include_str!("../quotes.csv");
const SUPPORT_DIR_NAME: &str = "Presence";
const ANNOUNCE_EVERY: Duration = Duration::from_secs(60);
const DENSE_UNTIL: Duration = Duration::from_secs(15 * 60);
const AFTER_DENSE_FIRST_GAP: Duration = Duration::from_secs(90);
const AFTER_DENSE_EVERY: Duration = Duration::from_secs(2 * 60);
const QUOTE_EVERY: Duration = Duration::from_secs(2 * 60);

fn main() -> io::Result<()> {
    ensure_support_files();
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

// ── quotes ────────────────────────────────────────────────────────────────────

fn model() -> String {
    std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:12b".into())
}

fn ollama_usable() -> bool {
    let output = match Command::new("ollama")
        .arg("list")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    list_has_model(&String::from_utf8_lossy(&output.stdout), &model())
}

fn list_has_model(stdout: &str, name: &str) -> bool {
    stdout
        .lines()
        .any(|line| match line.split_whitespace().next() {
            Some("NAME") | None => false,
            Some(found) => found == name,
        })
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

fn support_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(SUPPORT_DIR_NAME),
    )
}

/// First launch of a downloaded app has no checkout to edit. Seed the
/// Application Support copies from the text baked in at compile time.
fn ensure_support_files() {
    let Some(dir) = support_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    seed_if_missing(&dir.join("prompt.txt"), BUNDLED_PROMPT);
    seed_if_missing(&dir.join("quotes.csv"), BUNDLED_QUOTES);
}

fn seed_if_missing(path: &Path, bundled: &str) {
    if path.exists() {
        return;
    }
    let _ = std::fs::write(path, bundled);
}

fn read_text_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .filter(|text| !text.trim().is_empty())
}

/// Application Support, then the checkout, then the baked-in copy.
fn read_sidecar(support: Option<&Path>, checkout: &str, bundled: &str) -> String {
    if let Some(path) = support {
        if let Some(text) = read_text_file(path) {
            return text;
        }
    }
    read_or_bundled(checkout, bundled)
}

fn read_or_bundled(path: &str, bundled: &str) -> String {
    read_text_file(Path::new(path)).unwrap_or_else(|| bundled.to_string())
}

fn load_prompt() -> String {
    let support = support_dir().map(|dir| dir.join("prompt.txt"));
    read_sidecar(support.as_deref(), PROMPT_PATH, BUNDLED_PROMPT)
}

fn load_quotes() -> Vec<String> {
    let support = support_dir().map(|dir| dir.join("quotes.csv"));
    parse_quotes_csv(&read_sidecar(
        support.as_deref(),
        QUOTES_PATH,
        BUNDLED_QUOTES,
    ))
}

#[cfg(test)]
fn parse_quotes_md(src: &str) -> Vec<String> {
    let mut quotes = Vec::new();
    let mut current: Option<String> = None;
    for line in src.lines() {
        if let Some(rest) = numbered_quote_line(line) {
            if let Some(quote) = current.take() {
                quotes.push(quote);
            }
            current = Some(rest);
            continue;
        }
        if let Some(quote) = current.as_mut() {
            let trimmed = line.trim();
            if line.starts_with(' ') && !trimmed.is_empty() {
                quote.push(' ');
                quote.push_str(trimmed);
            }
        }
    }
    if let Some(quote) = current {
        quotes.push(quote);
    }
    quotes
}

#[cfg(test)]
fn numbered_quote_line(line: &str) -> Option<String> {
    let (num, rest) = line.split_once(". ")?;
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(rest.trim().to_string())
}

fn parse_quotes_csv(src: &str) -> Vec<String> {
    let mut quotes = Vec::new();
    let mut seen_header = false;
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let text = unquote_csv_field(line);
        if text.is_empty() {
            continue;
        }
        if !seen_header && text.eq_ignore_ascii_case("text") {
            seen_header = true;
            continue;
        }
        seen_header = true;
        quotes.push(text);
    }
    quotes
}

fn unquote_csv_field(line: &str) -> String {
    let line = line.trim();
    if let Some(inner) = line.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        inner.replace("\"\"", "\"")
    } else {
        line.to_string()
    }
}

fn pick_quote(quotes: &[String], index: usize) -> String {
    if quotes.is_empty() {
        return "quotes.csv is empty.".into();
    }
    quotes[index % quotes.len()].clone()
}

fn fallback_quote() -> String {
    pick_quote(&load_quotes(), 0)
}

enum QuoteLine {
    Model(String),
    List(String),
}

/// Ask Ollama for a quote in the background; the reply lands on `tx`.
fn request_quote(tx: Sender<QuoteLine>, previous: Option<String>) {
    thread::spawn(move || {
        let msg = match ollama_quote(previous.as_deref()) {
            Some(text) => QuoteLine::Model(text),
            None => QuoteLine::List(fallback_quote()),
        };
        let _ = tx.send(msg);
    });
}

fn ollama_quote(previous: Option<&str>) -> Option<String> {
    let mut prompt = load_prompt().trim().to_string();
    if prompt.is_empty() {
        return None;
    }
    if let Some(previous) = previous {
        prompt.push_str("\n\nPrevious intervention (do not repeat its wording or action):\n");
        prompt.push_str(previous);
    }
    let mut child = Command::new("ollama")
        .args(["run", "--think=false", &model()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

// ── cadence ───────────────────────────────────────────────────────────────────

/// Every minute through 15:00, then 16:30, then every 2 minutes.
fn latest_due(elapsed: Duration) -> Duration {
    if elapsed < ANNOUNCE_EVERY {
        return Duration::ZERO;
    }
    if elapsed < DENSE_UNTIL + AFTER_DENSE_FIRST_GAP {
        let mins = elapsed.as_secs() / ANNOUNCE_EVERY.as_secs();
        Duration::from_secs(mins.min(DENSE_UNTIL.as_secs() / 60) * 60)
    } else {
        let first = DENSE_UNTIL + AFTER_DENSE_FIRST_GAP;
        let steps = (elapsed - first).as_secs() / AFTER_DENSE_EVERY.as_secs();
        first + AFTER_DENSE_EVERY * (steps as u32)
    }
}

fn quote_with(at: Duration) -> bool {
    at >= DENSE_UNTIL || at.as_secs().is_multiple_of(QUOTE_EVERY.as_secs())
}

fn spoken_time(at: Duration) -> String {
    let mins = at.as_secs() / 60;
    let secs = at.as_secs() % 60;
    match (mins, secs) {
        (1, 0) => "1 minute".into(),
        (_, 0) => format!("{mins} minutes"),
        (1, _) => format!("1 minute {secs} seconds"),
        (_, _) => format!("{mins} minutes {secs} seconds"),
    }
}

// ── app ───────────────────────────────────────────────────────────────────────

struct App {
    running: bool,
    started_at: Instant,
    last_announced: Duration,
    quote: Option<String>,
    pending: bool,
    sleep_preventer: Option<Child>,
    speaker: Speaker,
    quote_tx: Sender<QuoteLine>,
    quote_rx: Receiver<QuoteLine>,
    quotes: Option<Vec<String>>,
    quote_index: usize,
}

impl App {
    fn new() -> Self {
        let quotes = if ollama_usable() {
            ensure_ollama();
            None
        } else {
            Some(load_quotes())
        };
        let (quote_tx, quote_rx) = mpsc::channel();
        let mut app = Self {
            running: false,
            started_at: Instant::now(),
            last_announced: Duration::ZERO,
            quote: None,
            pending: false,
            sleep_preventer: None,
            speaker: spawn_speaker(),
            quote_tx,
            quote_rx,
            quotes,
            quote_index: 0,
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
        self.last_announced = Duration::ZERO;
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
        if let Some(quotes) = &self.quotes {
            let q = pick_quote(quotes, self.quote_index);
            let _ = self.quote_tx.send(QuoteLine::List(q));
            return;
        }
        if self.pending {
            return;
        }
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
        if let Ok(line) = self.quote_rx.try_recv() {
            self.quote = Some(match line {
                QuoteLine::Model(q) => q,
                QuoteLine::List(q) => {
                    if self.quotes.is_none() {
                        self.quotes = Some(load_quotes());
                    }
                    q
                }
            });
            self.pending = false;
        }
        if !self.running {
            return;
        }
        let due = latest_due(self.elapsed());
        if due > self.last_announced {
            self.last_announced = due;
            self.speaker.send(spoken_time(due));
            if quote_with(due) {
                if let Some(q) = self.quote.clone() {
                    self.speaker.send(q);
                    if let Some(n) = self.quotes.as_ref().map(Vec::len).filter(|&n| n > 0) {
                        self.quote_index = (self.quote_index + 1) % n;
                    }
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

    #[test]
    fn parse_quotes_csv_skips_header_and_blanks() {
        let src = "\"text\"\n\n\"Hello, world.\"\n\"Second line.\"\n";
        assert_eq!(
            parse_quotes_csv(src),
            vec!["Hello, world.".to_string(), "Second line.".to_string()]
        );
    }

    #[test]
    fn parse_quotes_csv_unescapes_quotes() {
        let src = "text\n\"say \"\"hello\"\".\"\n";
        assert_eq!(parse_quotes_csv(src), vec!["say \"hello\".".to_string()]);
    }

    #[test]
    fn pick_quote_walks_in_order_and_loops() {
        let quotes = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(pick_quote(&quotes, 0), "a");
        assert_eq!(pick_quote(&quotes, 1), "b");
        assert_eq!(pick_quote(&quotes, 2), "c");
        assert_eq!(pick_quote(&quotes, 3), "a");
        assert_eq!(pick_quote(&quotes, 5), "c");
    }

    #[test]
    fn pick_quote_empty_is_calm() {
        assert_eq!(pick_quote(&[], 0), "quotes.csv is empty.");
    }

    #[test]
    fn read_or_bundled_falls_back_when_file_missing() {
        assert_eq!(
            read_or_bundled("/no/such/presence-sidecar.txt", "hello"),
            "hello"
        );
    }

    #[test]
    fn support_file_wins_over_checkout_and_bundled() {
        let dir = std::env::temp_dir().join(format!("presence-sidecar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let support = dir.join("prompt.txt");
        let checkout = dir.join("checkout.txt");
        std::fs::write(&support, "from support\n").unwrap();
        std::fs::write(&checkout, "from checkout\n").unwrap();
        assert_eq!(
            read_sidecar(Some(&support), checkout.to_str().unwrap(), "bundled"),
            "from support\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_support_file_falls_through_to_checkout() {
        let dir = std::env::temp_dir().join(format!("presence-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let support = dir.join("prompt.txt");
        let checkout = dir.join("checkout.txt");
        std::fs::write(&support, "  \n").unwrap();
        std::fs::write(&checkout, "from checkout\n").unwrap();
        assert_eq!(
            read_sidecar(Some(&support), checkout.to_str().unwrap(), "bundled"),
            "from checkout\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_if_missing_writes_once() {
        let dir = std::env::temp_dir().join(format!("presence-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("prompt.txt");
        seed_if_missing(&path, "first\n");
        seed_if_missing(&path, "second\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ollama_failure_falls_back_to_bundled_quotes() {
        let quotes = load_quotes();
        let q = fallback_quote();
        assert_eq!(q, quotes[0]);
        assert_ne!(q, "ollama is not available.");
    }

    #[test]
    fn bundled_quotes_csv_has_one_hundred_ten() {
        let src = std::fs::read_to_string(QUOTES_PATH).unwrap();
        assert_eq!(parse_quotes_csv(&src).len(), 110);
    }

    #[test]
    fn quotes_csv_matches_workshop_md() {
        let md_path = concat!(env!("CARGO_MANIFEST_DIR"), "/quotes/quotes.md");
        let md = std::fs::read_to_string(md_path).unwrap();
        let csv = std::fs::read_to_string(QUOTES_PATH).unwrap();
        assert_eq!(parse_quotes_md(&md), parse_quotes_csv(&csv));
    }

    #[test]
    fn list_has_model_reads_name_column() {
        let stdout = "NAME ID SIZE\ngemma4:12b abc 8GB\nllama3:latest def 2GB\n";
        assert!(list_has_model(stdout, "gemma4:12b"));
        assert!(!list_has_model(stdout, "mistral"));
        assert!(!list_has_model("NAME ID SIZE\n", "gemma4:12b"));
    }

    #[test]
    fn cadence_is_every_minute_through_fifteen() {
        assert_eq!(latest_due(Duration::from_secs(59)), Duration::ZERO);
        assert_eq!(latest_due(Duration::from_secs(60)), Duration::from_secs(60));
        assert_eq!(
            latest_due(Duration::from_secs(119)),
            Duration::from_secs(60)
        );
        assert_eq!(
            latest_due(Duration::from_secs(14 * 60)),
            Duration::from_secs(14 * 60)
        );
        assert_eq!(latest_due(Duration::from_secs(15 * 60)), DENSE_UNTIL);
        assert_eq!(latest_due(Duration::from_secs(16 * 60 + 29)), DENSE_UNTIL);
    }

    #[test]
    fn cadence_steps_to_ninety_seconds_then_every_two_minutes() {
        let sixteen_thirty = Duration::from_secs(16 * 60 + 30);
        let eighteen_thirty = Duration::from_secs(18 * 60 + 30);
        assert_eq!(latest_due(sixteen_thirty), sixteen_thirty);
        assert_eq!(
            latest_due(Duration::from_secs(18 * 60 + 29)),
            sixteen_thirty
        );
        assert_eq!(latest_due(eighteen_thirty), eighteen_thirty);
        assert_eq!(
            latest_due(Duration::from_secs(20 * 60 + 30)),
            Duration::from_secs(20 * 60 + 30)
        );
    }

    #[test]
    fn quotes_stay_every_two_minutes_then_follow_the_new_cadence() {
        assert!(quote_with(Duration::from_secs(2 * 60)));
        assert!(!quote_with(Duration::from_secs(3 * 60)));
        assert!(quote_with(DENSE_UNTIL));
        assert!(quote_with(Duration::from_secs(16 * 60 + 30)));
    }

    #[test]
    fn spoken_time_names_minutes_and_seconds() {
        assert_eq!(spoken_time(Duration::from_secs(60)), "1 minute");
        assert_eq!(spoken_time(Duration::from_secs(15 * 60)), "15 minutes");
        assert_eq!(
            spoken_time(Duration::from_secs(16 * 60 + 30)),
            "16 minutes 30 seconds"
        );
    }
}
