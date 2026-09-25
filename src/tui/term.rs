//! The terminal for the TUI: raw mode and the alternate screen, put back
//! on exit and on a panic, and a frame of styled rows drawn in one go. The
//! colours are seaof.glass's (Lasimeri/lasimeri.github.io, `index.html`'s
//! `:root`): copper on near-black, a dimmer copper for what is secondary,
//! the surface colour under the selected row, the border colour for rules
//! and the empty part of a bar. No widget library: `crossterm` for raw mode,
//! keys and escape sequences, the layout here. See term.md.

use std::io::{self, Stdout, Write};

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
    pub bold: bool,
    pub italic: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, fg: Color) -> Self {
        Self {
            text: text.into(),
            fg,
            bold: false,
            italic: false,
        }
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

    /// Set row `r` (ignored past the bottom).
    pub fn set(&mut self, r: u16, spans: Vec<Span>, bg: Color) {
        if let Some(row) = self.rows.get_mut(r as usize) {
            *row = Row { spans, bg };
        }
    }
}

/// The terminal, in raw mode on the alternate screen while this lives.
pub struct Term {
    out: Stdout,
}

impl Term {
    pub fn open() -> io::Result<Term> {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
        }));
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(
            out,
            terminal::EnterAlternateScreen,
            event::EnableBracketedPaste,
            cursor::Hide
        )?;
        Ok(Term { out })
    }

    pub fn size() -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    pub fn draw(&mut self, f: &Frame) -> io::Result<()> {
        queue!(self.out, terminal::BeginSynchronizedUpdate, cursor::Hide)?;
        for (r, row) in f.rows.iter().enumerate() {
            queue!(
                self.out,
                cursor::MoveTo(0, r as u16),
                SetBackgroundColor(row.bg)
            )?;
            let mut left = f.width as usize;
            for s in &row.spans {
                if left == 0 {
                    break;
                }
                let text: String = s.text.chars().take(left).collect();
                left -= text.chars().count();
                queue!(self.out, SetForegroundColor(s.fg))?;
                if s.bold {
                    queue!(self.out, SetAttribute(Attribute::Bold))?;
                }
                if s.italic {
                    queue!(self.out, SetAttribute(Attribute::Italic))?;
                }
                queue!(
                    self.out,
                    Print(text),
                    SetAttribute(Attribute::Reset),
                    SetBackgroundColor(row.bg)
                )?;
            }
            queue!(self.out, Print(" ".repeat(left)))?;
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
