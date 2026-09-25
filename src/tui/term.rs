//! The terminal for the TUI: raw mode and the alternate screen, put back
//! on exit and on a panic, and a frame of styled rows drawn in one go. The
//! colours are seaof.glass's (Lasimeri/lasimeri.github.io, `index.html`'s
//! `:root`): copper on near-black, a dimmer copper for what is secondary,
//! the surface colour under the selected row, the border colour for rules
//! and the empty part of a bar. No widget library: `crossterm` for raw mode,
//! keys and escape sequences, the layout here. See term.md.

use std::io::{self, Stdout, Write};
use std::sync::Mutex;

use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::{cursor, event, execute, queue, terminal};

/// seaof.glass's palette.
pub mod theme {
    use crossterm::style::Color;

    /// `--bg: #0a0a0f`
    pub const BG: Color = Color::Rgb {
        r: 0x0a,
        g: 0x0a,
        b: 0x0f,
    };
    /// `--surface: #12121a`, under the selected row
    pub const SURFACE: Color = Color::Rgb {
        r: 0x12,
        g: 0x12,
        b: 0x1a,
    };
    /// `--border: #1e1e2e`, rules and the empty part of a bar
    pub const BORDER: Color = Color::Rgb {
        r: 0x1e,
        g: 0x1e,
        b: 0x2e,
    };
    /// `--text` and `--accent: #c4945a`
    pub const TEXT: Color = Color::Rgb {
        r: 0xc4,
        g: 0x94,
        b: 0x5a,
    };
    /// `--text-dim: #8a6a3e`
    pub const DIM: Color = Color::Rgb {
        r: 0x8a,
        g: 0x6a,
        b: 0x3e,
    };
    /// `--accent-dim: #7a5c38`
    pub const ACCENT_DIM: Color = Color::Rgb {
        r: 0x7a,
        g: 0x5c,
        b: 0x38,
    };
}

/// A run of text in one style.
#[derive(Debug, Clone)]
pub struct Span {
    pub text: String,
    pub fg: Color,
    /// The colour under this run, when not the row's.
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, fg: Color) -> Self {
        Self {
            text: text.into(),
            fg,
            bg: None,
            bold: false,
            italic: false,
        }
    }

    pub fn on(mut self, bg: Color) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

/// One row: its spans and the colour under all of it.
#[derive(Debug, Clone)]
pub struct Row {
    pub spans: Vec<Span>,
    pub bg: Color,
}

/// A whole screen, drawn in one go.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub rows: Vec<Row>,
    /// Where the text cursor shows, if anywhere.
    pub cursor: Option<(u16, u16)>,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            rows: (0..height)
                .map(|_| Row {
                    spans: Vec::new(),
                    bg: theme::BG,
                })
                .collect(),
            cursor: None,
        }
    }

    /// Set row `r` (ignored past the bottom), cut to the frame's width.
    /// Control characters (a loaded file's tabs, a log's `\r`, an escape
    /// sequence in an error) would move the terminal's cursor or change
    /// its colours, so each is drawn as one cell: a space for whitespace,
    /// `�` for anything else.
    pub fn set(&mut self, r: u16, spans: Vec<Span>, bg: Color) {
        let mut left = self.width as usize;
        let mut kept = Vec::with_capacity(spans.len());
        for mut s in spans {
            if left == 0 {
                break;
            }
            if s.text.chars().any(char::is_control) {
                s.text = s
                    .text
                    .chars()
                    .map(|c| match c {
                        '\t' | '\r' | '\n' => ' ',
                        c if c.is_control() => '\u{FFFD}',
                        c => c,
                    })
                    .collect();
            }
            let n = s.text.chars().count();
            if n > left {
                s.text = s.text.chars().take(left).collect();
            }
            left -= n.min(left);
            kept.push(s);
        }
        if let Some(row) = self.rows.get_mut(r as usize) {
            *row = Row { spans: kept, bg };
        }
    }

    /// Characters in row `r`.
    pub fn row_width(&self, r: usize) -> usize {
        self.rows.get(r).map_or(0, |row| {
            row.spans.iter().map(|s| s.text.chars().count()).sum()
        })
    }

    /// Row `r`'s text, unstyled.
    pub fn row_text(&self, r: usize) -> String {
        self.rows.get(r).map_or_else(String::new, |row| {
            row.spans.iter().map(|s| s.text.as_str()).collect()
        })
    }
}

/// How colours are drawn: seaof.glass's truecolor (the default); the
/// nearest of the 256-colour palette (`MJEV_COLOR=256`, for a terminal
/// without truecolor); or none (`NO_COLOR` set, as no-color.org asks, or
/// `MJEV_COLOR=none`), the selected row then in reverse video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colors {
    True,
    Ansi256,
    None,
}

impl Colors {
    pub fn from_env() -> Self {
        if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Colors::None;
        }
        match std::env::var("MJEV_COLOR").as_deref() {
            Ok("256") => Colors::Ansi256,
            Ok("none") => Colors::None,
            _ => Colors::True,
        }
    }

    /// `c` as drawn, `None` for no colour at all.
    fn map(self, c: Color) -> Option<Color> {
        match (self, c) {
            (Colors::None, _) => None,
            (Colors::Ansi256, Color::Rgb { r, g, b }) => Some(Color::AnsiValue(ansi256(r, g, b))),
            (_, c) => Some(c),
        }
    }
}

/// The xterm 256-colour index nearest `(r, g, b)`: the 6x6x6 cube or the
/// grey ramp, whichever is closer.
pub fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let near = |v: u8| {
        (0..6)
            .min_by_key(|&i| (LEVELS[i] - v as i32).abs())
            .unwrap_or(0)
    };
    let dist = |x: i32, y: i32, z: i32| {
        (x - r as i32).pow(2) + (y - g as i32).pow(2) + (z - b as i32).pow(2)
    };
    let (ri, gi, bi) = (near(r), near(g), near(b));
    let cube = dist(LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let grey = ((r as i32 + g as i32 + b as i32) / 3 - 8).clamp(0, 230) / 10;
    let gv = 8 + 10 * grey;
    if dist(gv, gv, gv) < cube {
        232 + grey as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

/// The terminal, in raw mode on the alternate screen while this lives.
pub struct Term {
    out: Stdout,
    colors: Colors,
}

/// A worker thread's last panic, kept instead of printed (it would land on
/// the screen); `take_worker_panic` hands it to whoever reports it.
static WORKER_PANIC: Mutex<Option<String>> = Mutex::new(None);

pub fn take_worker_panic() -> Option<String> {
    WORKER_PANIC.lock().ok()?.take()
}

impl Term {
    /// Raw mode and the alternate screen. A panic on this thread (the one
    /// that draws) puts the terminal back before it is reported; a panic
    /// on any other thread leaves the screen alone and is kept for
    /// `take_worker_panic`, since drawing goes on.
    pub fn open() -> io::Result<Term> {
        let ui = std::thread::current().id();
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().id() == ui {
                restore();
                prev(info);
            } else if let Ok(mut slot) = WORKER_PANIC.lock() {
                *slot = Some(info.to_string());
            }
        }));
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(
            out,
            terminal::EnterAlternateScreen,
            event::EnableBracketedPaste,
            cursor::Hide,
            terminal::SetTitle("mjev")
        )?;
        Ok(Term {
            out,
            colors: Colors::from_env(),
        })
    }

    pub fn size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    /// Start a run of text: attributes reset, then its colours (as the
    /// palette allows) or, with no colour, reverse video for a highlight.
    fn style(&mut self, fg: Color, bg: Color) -> io::Result<()> {
        queue!(self.out, SetAttribute(Attribute::Reset), ResetColor)?;
        if let Some(c) = self.colors.map(bg) {
            queue!(self.out, SetBackgroundColor(c))?;
        } else if bg != theme::BG {
            queue!(self.out, SetAttribute(Attribute::Reverse))?;
        }
        if let Some(c) = self.colors.map(fg) {
            queue!(self.out, SetForegroundColor(c))?;
        }
        Ok(())
    }

    pub fn draw(&mut self, f: &Frame) -> io::Result<()> {
        queue!(self.out, terminal::BeginSynchronizedUpdate, cursor::Hide)?;
        for (r, row) in f.rows.iter().enumerate() {
            queue!(self.out, cursor::MoveTo(0, r as u16))?;
            let mut left = f.width as usize;
            for s in &row.spans {
                if left == 0 {
                    break;
                }
                let text: String = s.text.chars().take(left).collect();
                left -= text.chars().count();
                self.style(s.fg, s.bg.unwrap_or(row.bg))?;
                if s.bold {
                    queue!(self.out, SetAttribute(Attribute::Bold))?;
                }
                if s.italic {
                    queue!(self.out, SetAttribute(Attribute::Italic))?;
                }
                queue!(self.out, Print(text))?;
            }
            self.style(theme::TEXT, row.bg)?;
            queue!(
                self.out,
                Print(" ".repeat(left)),
                SetAttribute(Attribute::Reset)
            )?;
        }
        queue!(self.out, ResetColor)?;
        if let Some((x, y)) = f.cursor {
            queue!(self.out, cursor::MoveTo(x, y), cursor::Show)?;
        }
        queue!(self.out, terminal::EndSynchronizedUpdate)?;
        self.out.flush()
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        restore();
    }
}

/// Put the terminal back: cooked mode, the main screen, the cursor.
fn restore() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        ResetColor,
        event::DisableBracketedPaste,
        terminal::LeaveAlternateScreen,
        cursor::Show
    );
}

#[cfg(test)]
mod tests {
    use super::ansi256;

    #[test]
    fn the_palette_maps_to_its_nearest_256_colours() {
        assert_eq!(ansi256(0, 0, 0), 16);
        assert_eq!(ansi256(255, 255, 255), 231);
        assert_eq!(ansi256(0x0a, 0x0a, 0x0f), 232); // BG: the darkest grey
        assert_eq!(ansi256(0xc4, 0x94, 0x5a), 173); // TEXT: (215, 135, 95)
        assert_eq!(ansi256(0x12, 0x12, 0x1a), 233); // SURFACE, a step above BG
    }
}
