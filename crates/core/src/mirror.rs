//! A terminal without a screen that processes the session's output, so terminal queries from the
//! agent are answered even when no host window is attached (design decision 5).
//!
//! When the host's window is attached, its terminal answers queries through the host's input,
//! and the mirror's replies are dropped. Viewer terminals never answer. Either way exactly one
//! answer reaches the agent.

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
pub use alacritty_terminal::vte::ansi::Rgb;

/// Cell size reported for text-area size queries; there is no real screen.
const CELL_WIDTH: u16 = 8;
const CELL_HEIGHT: u16 = 16;

/// Colours reported for colour queries. Agents use the background to choose a light or dark
/// theme, so the host window reports its theme's colours when they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub foreground: Rgb,
    pub background: Rgb,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            foreground: Rgb { r: 0xd4, g: 0xd4, b: 0xd4 },
            background: Rgb { r: 0x1e, g: 0x1e, b: 0x1e },
        }
    }
}

#[derive(Clone)]
struct Replies {
    pending: Arc<Mutex<Vec<u8>>>,
    colors: Arc<Mutex<Colors>>,
    size: Arc<Mutex<(u16, u16)>>,
}

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        let reply = match event {
            Event::PtyWrite(text) => text,
            Event::ColorRequest(index, format) => format(self.color(index)),
            Event::TextAreaSizeRequest(format) => {
                let (cols, rows) = *self.size.lock().unwrap();
                format(WindowSize {
                    num_cols: cols,
                    num_lines: rows,
                    cell_width: CELL_WIDTH,
                    cell_height: CELL_HEIGHT,
                })
            }
            _ => return,
        };
        self.pending.lock().unwrap().extend_from_slice(reply.as_bytes());
    }
}

impl Replies {
    /// Colour for an OSC 4/10/11 query: indices 0-255 are the palette, 256 is the foreground
    /// and 257 the background, as alacritty numbers them.
    fn color(&self, index: usize) -> Rgb {
        let colors = *self.colors.lock().unwrap();
        match index {
            256 => colors.foreground,
            257 => colors.background,
            index => xterm_palette(index.min(255) as u8),
        }
    }
}

pub struct Mirror {
    term: Term<Replies>,
    parser: Processor,
    replies: Replies,
}

impl Mirror {
    pub fn new(cols: u16, rows: u16) -> Self {
        let replies = Replies {
            pending: Arc::default(),
            colors: Arc::default(),
            size: Arc::new(Mutex::new((cols, rows))),
        };
        let size = TermSize::new(cols.into(), rows.into());
        Self {
            term: Term::new(Config::default(), &size, replies.clone()),
            parser: Processor::new(),
            replies,
        }
    }

    /// Processes agent output and returns the replies the terminal would send back.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        std::mem::take(&mut *self.replies.pending.lock().unwrap())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        *self.replies.size.lock().unwrap() = (cols, rows);
        self.term.resize(TermSize::new(cols.into(), rows.into()));
    }

    pub fn set_colors(&mut self, colors: Colors) {
        *self.replies.colors.lock().unwrap() = colors;
    }
}

/// Who answers terminal queries from the agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Responder {
    /// The host's window is attached; its terminal answers through the host's input.
    HostWindow,
    /// No host window; the mirror answers.
    Mirror,
}

impl Responder {
    /// The mirror's replies that should reach the agent.
    pub fn filter(self, mirror_replies: Vec<u8>) -> Option<Vec<u8>> {
        (self == Responder::Mirror && !mirror_replies.is_empty()).then_some(mirror_replies)
    }
}

/// The standard xterm 256-colour palette.
fn xterm_palette(index: u8) -> Rgb {
    const BASE: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x00, 0x00),
        (0x00, 0xcd, 0x00),
        (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee),
        (0xcd, 0x00, 0xcd),
        (0x00, 0xcd, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];
    match index {
        0..16 => {
            let (r, g, b) = BASE[index as usize];
            Rgb { r, g, b }
        }
        16..232 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Rgb { r: level(i / 36), g: level(i / 6 % 6), b: level(i % 6) }
        }
        _ => {
            let v = 8 + (index - 232) * 10;
            Rgb { r: v, g: v, b: v }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_device_attributes() {
        let mut mirror = Mirror::new(80, 24);
        let reply = mirror.feed(b"hello\x1b[c");
        assert!(reply.starts_with(b"\x1b[?"), "{:?}", String::from_utf8_lossy(&reply));
        assert!(reply.ends_with(b"c"));
        assert!(mirror.feed(b"more output").is_empty());
    }

    #[test]
    fn answers_cursor_position() {
        let mut mirror = Mirror::new(80, 24);
        assert_eq!(mirror.feed(b"\x1b[5;10H\x1b[6n"), b"\x1b[5;10R");
    }

    #[test]
    fn answers_background_colour_with_the_host_theme() {
        let mut mirror = Mirror::new(80, 24);
        mirror.set_colors(Colors {
            foreground: Rgb { r: 0, g: 0, b: 0 },
            background: Rgb { r: 0xff, g: 0xff, b: 0xff },
        });
        let reply = String::from_utf8(mirror.feed(b"\x1b]11;?\x07")).unwrap();
        assert!(reply.contains("rgb:ffff/ffff/ffff"), "{reply}");
    }

    #[test]
    fn answers_text_area_size_after_resize() {
        let mut mirror = Mirror::new(80, 24);
        mirror.resize(100, 30);
        assert_eq!(mirror.feed(b"\x1b[18t"), b"\x1b[8;30;100t");
    }

    #[test]
    fn replies_reach_the_agent_only_without_a_host_window() {
        let reply = b"\x1b[?6c".to_vec();
        assert_eq!(Responder::Mirror.filter(reply.clone()), Some(reply.clone()));
        assert_eq!(Responder::HostWindow.filter(reply), None);
        assert_eq!(Responder::Mirror.filter(Vec::new()), None);
    }

    /// An agent in a real pty asks for device attributes while no host window is attached, and
    /// must get exactly one answer.
    #[cfg(unix)]
    #[test]
    fn agent_without_host_window_gets_exactly_one_reply() {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};

        use specdeck_protocol::AgentId;

        use crate::environment::Environment;
        use crate::launcher::{LaunchOptions, launch};
        use crate::test_support::fake_executable;

        let bin = tempfile::tempdir().unwrap();
        // Reads replies ending in "c": the first must arrive, a second must not.
        fake_executable(
            bin.path(),
            "claude",
            r#"exec bash -c 'stty raw -echo
printf "\033[c"
IFS= read -r -t 3 -d c first
IFS= read -r -t 1 -d c second
stty sane
printf "\nfirst=[%s] second=[%s] done\n" "${first#?}" "$second"'"#,
        );
        let path = format!("{}:/usr/bin:/bin", bin.path().display());
        let env: Environment = [("PATH", path)].into_iter().collect();
        let options = LaunchOptions {
            agent: AgentId::ClaudeCode,
            folder: bin.path().to_owned(),
            cols: 80,
            rows: 24,
            extra_args: vec![],
            extra_env: vec![],
            program: None,
        };
        let mut process = launch(&options, &env).unwrap();
        let mut mirror = Mirror::new(80, 24);

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut output = Vec::new();
        let mut buf = [0; 1024];
        while Instant::now() < deadline && !String::from_utf8_lossy(&output).contains("done") {
            let n = process.reader.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
            if let Some(reply) = Responder::Mirror.filter(mirror.feed(&buf[..n])) {
                process.writer.write_all(&reply).unwrap();
            }
        }

        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("first=[[?6] second=[] done"), "{output:?}");
    }

    #[test]
    fn palette_matches_xterm() {
        assert_eq!(xterm_palette(1), Rgb { r: 0xcd, g: 0, b: 0 });
        assert_eq!(xterm_palette(196), Rgb { r: 0xff, g: 0, b: 0 });
        assert_eq!(xterm_palette(244), Rgb { r: 0x80, g: 0x80, b: 0x80 });
    }
}
