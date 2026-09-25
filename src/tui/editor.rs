//! Text editing for the TUI: a buffer of lines and a cursor, with the
//! operations a terminal's keys map to. No terminal here: the TUI draws
//! what `visible` returns and moves the cursor where `cursor_on_screen`
//! says. `single_line` editors ignore newlines (a pasted newline becomes a
//! space). See editor.md.

#[derive(Debug, Clone)]
pub struct Editor {
    lines: Vec<Vec<char>>,
    /// Cursor: line, and column in characters.
    row: usize,
    col: usize,
    /// The first line and column shown.
    top: usize,
    left: usize,
    single_line: bool,
}

impl Editor {
    pub fn new(single_line: bool) -> Self {
        Self {
            lines: vec![Vec::new()],
            row: 0,
            col: 0,
            top: 0,
            left: 0,
            single_line,
        }
    }

    pub fn with_text(text: &str, single_line: bool) -> Self {
        let mut e = Self::new(single_line);
        e.set_text(text);
        e
    }

    /// Replace the text; the cursor goes to its end.
    pub fn set_text(&mut self, text: &str) {
        let text = if self.single_line {
            text.replace(['\r', '\n'], " ")
        } else {
            text.replace("\r\n", "\n")
        };
        self.lines = text.split('\n').map(|l| l.chars().collect()).collect();
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        self.row = self.lines.len() - 1;
        self.col = self.lines[self.row].len();
        self.top = 0;
        self.left = 0;
    }

    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            return self.newline();
        }
        let c = if c == '\t' { ' ' } else { c };
        self.lines[self.row].insert(self.col, c);
        self.col += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.replace("\r\n", "\n").chars() {
            if c == '\n' && self.single_line {
                self.insert_char(' ');
            } else {
                self.insert_char(c);
            }
        }
    }

    pub fn newline(&mut self) {
        if self.single_line {
            return;
        }
        let rest = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.lines[self.row].remove(self.col);
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].extend(line);
        }
    }

    pub fn delete(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.lines[self.row].remove(self.col);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].extend(next);
        }
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
    }

    pub fn right(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = self.lines[self.row].len();
    }

    pub fn page(&mut self, rows: isize) {
        for _ in 0..rows.unsigned_abs() {
            if rows < 0 {
                self.up();
            } else {
                self.down();
            }
        }
    }

    /// Scroll so the cursor is inside a `width` x `height` view, and
    /// return the visible slice of every visible line.
    pub fn visible(&mut self, width: usize, height: usize) -> Vec<String> {
        let (width, height) = (width.max(1), height.max(1));
        if self.row < self.top {
            self.top = self.row;
        } else if self.row >= self.top + height {
            self.top = self.row + 1 - height;
        }
        if self.col < self.left {
            self.left = self.col;
        } else if self.col >= self.left + width {
            self.left = self.col + 1 - width;
        }
        self.lines
            .iter()
            .skip(self.top)
            .take(height)
            .map(|l| l.iter().skip(self.left).take(width).collect())
            .collect()
    }

    /// Where the cursor is inside the last `visible` view.
    pub fn cursor_on_screen(&self) -> (usize, usize) {
        (self.row - self.top, self.col - self.left)
    }
}

#[cfg(test)]
mod tests {
    use super::Editor;

    #[test]
    fn typing_joining_and_splitting_lines() {
        let mut e = Editor::new(false);
        e.insert_str("ab\ncd");
        assert_eq!(e.text(), "ab\ncd");
        e.home();
        e.backspace(); // joins "cd" onto "ab"
        assert_eq!((e.text().as_str(), e.cursor()), ("abcd", (0, 2)));
        e.newline();
        assert_eq!(e.text(), "ab\ncd");
        e.up();
        e.end();
        e.delete(); // joins the next line
        assert_eq!(e.text(), "abcd");
    }

    #[test]
    fn a_single_line_editor_keeps_one_line() {
        let mut e = Editor::new(true);
        e.insert_str("first\nsecond");
        e.newline();
        assert_eq!(e.text(), "first second");
    }

    #[test]
    fn the_view_follows_the_cursor() {
        let mut e = Editor::with_text("0123456789\nx\ny\nz", false);
        e.up();
        e.up();
        e.up();
        e.end();
        let v = e.visible(4, 2);
        assert_eq!(v, vec!["789".to_string(), "".to_string()]);
        assert_eq!(e.cursor_on_screen(), (0, 3));
    }
}
