//! Text editing for the TUI: a buffer of lines and a cursor, with the
//! operations a terminal's keys map to. No terminal here: the TUI draws
//! what `visible` returns and moves the cursor where `cursor_on_screen`
//! says. A multi-line editor wraps its lines to the view's width (at a
//! space when one fits, else mid-word), and up and down move by the rows
//! as drawn. `single_line` editors ignore newlines (a pasted newline
//! becomes a space) and scroll sideways instead. See editor.md.

#[derive(Debug, Clone)]
pub struct Editor {
    lines: Vec<Vec<char>>,
    /// Cursor: line, and column in characters.
    row: usize,
    col: usize,
    /// The first row shown (a line when not wrapping, a drawn row when
    /// wrapping) and, when not wrapping, the first column.
    top: usize,
    left: usize,
    single_line: bool,
    /// The width lines wrap at, from the last `visible` (0: none yet).
    width: usize,
    /// The column up and down aim for, kept across a run of them.
    goal: Option<usize>,
    /// The cursor's place in the last view, when wrapping.
    screen: (usize, usize),
    /// The text and cursor before each undoable step, newest last.
    history: Vec<(Vec<Vec<char>>, usize, usize)>,
    /// The kind of the step in progress: a run of one kind is one step.
    step: Option<Step>,
}

/// What an edit was, so a run of typing (up to a space) or of erasing
/// undoes as one step; anything else is a step of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Type,
    Erase,
    Other,
}

/// The most steps `undo` goes back.
const HISTORY: usize = 100;

/// Where each drawn row of a line starts, wrapped at `w` columns: after the
/// last space that fits, else at `w` (a word longer than the row). A space
/// right after a full row hangs on it, in the column `visible` keeps free,
/// rather than starting the next row by itself.
fn breaks(l: &[char], w: usize) -> Vec<usize> {
    let w = w.max(1);
    let mut starts = vec![0];
    let mut s = 0;
    while l.len() - s > w {
        let cut = if l[s + w] == ' ' {
            s + w + 1
        } else {
            (s + 1..=s + w)
                .rev()
                .find(|&p| l[p - 1] == ' ')
                .unwrap_or(s + w)
        };
        starts.push(cut);
        s = cut;
    }
    starts
}

/// The drawn row holding (`row`, `col`), and the column in it. A column at
/// a break belongs to the row that starts there.
fn locate(rows: &[(usize, usize, usize)], row: usize, col: usize) -> (usize, usize) {
    let i = rows
        .iter()
        .rposition(|&(r, s, _)| r == row && s <= col)
        .unwrap_or(0);
    (i, col - rows[i].1)
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
            width: 0,
            goal: None,
            screen: (0, 0),
            history: Vec::new(),
            step: None,
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
        self.goal = None;
        self.history.clear();
        self.step = None;
    }

    /// The cursor to the very start, the view with it (a loaded text reads
    /// from its top).
    pub fn to_start(&mut self) {
        self.row = 0;
        self.col = 0;
        self.top = 0;
        self.left = 0;
        self.moved();
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

    fn wraps(&self) -> bool {
        !self.single_line
    }

    /// The drawn rows at wrap width `w`: (line, start, end).
    fn rows(&self, w: usize) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            let b = breaks(l, w);
            for (k, &s) in b.iter().enumerate() {
                out.push((i, s, b.get(k + 1).copied().unwrap_or(l.len())));
            }
        }
        out
    }

    /// How many rows the text takes in a view `width` wide.
    pub fn rows_at(&self, width: usize) -> usize {
        if self.wraps() {
            self.rows((width.max(2)) - 1).len()
        } else {
            self.lines.len()
        }
    }

    /// Before an edit: a snapshot, unless it continues the step in
    /// progress.
    fn record(&mut self, step: Step) {
        if step == Step::Other || self.step != Some(step) {
            self.history.push((self.lines.clone(), self.row, self.col));
            if self.history.len() > HISTORY {
                self.history.remove(0);
            }
        }
        self.step = Some(step);
        self.goal = None;
    }

    /// The last step taken back; whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some((lines, row, col)) = self.history.pop() else {
            return false;
        };
        (self.lines, self.row, self.col) = (lines, row, col);
        self.step = None;
        self.goal = None;
        true
    }

    /// A move ends the step in progress.
    fn moved(&mut self) {
        self.step = None;
        self.goal = None;
    }

    fn put(&mut self, c: char) {
        let c = if c == '\t' { ' ' } else { c };
        self.lines[self.row].insert(self.col, c);
        self.col += 1;
    }

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            return self.newline();
        }
        self.record(Step::Type);
        self.put(c);
        if c == ' ' {
            // A word typed is a step.
            self.step = None;
        }
    }

    /// A paste: one step, however long.
    pub fn insert_str(&mut self, s: &str) {
        self.record(Step::Other);
        for c in s.replace("\r\n", "\n").chars() {
            match c {
                '\n' if self.single_line => self.put(' '),
                '\n' => self.split(),
                c => self.put(c),
            }
        }
        self.step = None;
    }

    pub fn newline(&mut self) {
        if self.single_line {
            return;
        }
        self.record(Step::Other);
        self.split();
    }

    fn split(&mut self) {
        let rest = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col == 0 && self.row == 0 {
            return;
        }
        self.record(Step::Erase);
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
        if self.col == self.lines[self.row].len() && self.row + 1 == self.lines.len() {
            return;
        }
        self.record(Step::Erase);
        if self.col < self.lines[self.row].len() {
            self.lines[self.row].remove(self.col);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].extend(next);
        }
    }

    /// Where the word before the cursor starts: back over spaces, then
    /// over the word.
    fn word_start(&self) -> usize {
        let l = &self.lines[self.row];
        let mut c = self.col;
        while c > 0 && l[c - 1] == ' ' {
            c -= 1;
        }
        while c > 0 && l[c - 1] != ' ' {
            c -= 1;
        }
        c
    }

    /// To the start of this word or the one before (the line before's end
    /// from a line's start).
    pub fn word_left(&mut self) {
        if self.col == 0 {
            return self.left();
        }
        self.col = self.word_start();
        self.moved();
    }

    /// Past the end of this word or the next.
    pub fn word_right(&mut self) {
        let l = &self.lines[self.row];
        if self.col == l.len() {
            return self.right();
        }
        let mut c = self.col;
        while c < l.len() && l[c] == ' ' {
            c += 1;
        }
        while c < l.len() && l[c] != ' ' {
            c += 1;
        }
        self.col = c;
        self.moved();
    }

    /// The word before the cursor erased (at a line's start, the line
    /// joined to the one before).
    pub fn delete_word_back(&mut self) {
        if self.col == 0 {
            return self.backspace();
        }
        self.record(Step::Other);
        let s = self.word_start();
        self.lines[self.row].drain(s..self.col);
        self.col = s;
    }

    /// The rest of the line erased (at its end, the next line joined).
    pub fn kill_to_end(&mut self) {
        if self.col == self.lines[self.row].len() {
            return self.delete();
        }
        self.record(Step::Other);
        self.lines[self.row].truncate(self.col);
    }

    /// The line before the cursor erased.
    pub fn kill_to_start(&mut self) {
        if self.col == 0 {
            return;
        }
        self.record(Step::Other);
        self.lines[self.row].drain(..self.col);
        self.col = 0;
    }

    /// The cursor to the very end of the text.
    pub fn to_end(&mut self) {
        self.row = self.lines.len() - 1;
        self.col = self.lines[self.row].len();
        self.moved();
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
        self.moved();
    }

    pub fn right(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
        self.moved();
    }

    pub fn up(&mut self) {
        self.vertical(-1);
    }

    pub fn down(&mut self) {
        self.vertical(1);
    }

    /// One row up or down: a drawn row when wrapping (once a view has set
    /// the width), else a line. The column aimed for is kept across a run
    /// of these, so passing a short row does not pull the cursor left.
    fn vertical(&mut self, by: isize) {
        self.step = None;
        if self.wraps() && self.width > 0 {
            let rows = self.rows(self.width);
            let (i, c) = locate(&rows, self.row, self.col);
            let goal = *self.goal.get_or_insert(c);
            let Some(t) = i.checked_add_signed(by).filter(|&t| t < rows.len()) else {
                return;
            };
            let (r, s, e) = rows[t];
            // A column at the end of a row that is not its line's last is
            // the next row's start; stop one short of it.
            let last = rows.get(t + 1).is_none_or(|n| n.0 != r);
            let max = if last { e } else { e - 1 };
            self.row = r;
            self.col = (s + goal).min(max);
            return;
        }
        let goal = *self.goal.get_or_insert(self.col);
        let Some(r) = self
            .row
            .checked_add_signed(by)
            .filter(|&r| r < self.lines.len())
        else {
            return;
        };
        self.row = r;
        self.col = goal.min(self.lines[r].len());
    }

    pub fn home(&mut self) {
        self.col = 0;
        self.moved();
    }

    pub fn end(&mut self) {
        self.col = self.lines[self.row].len();
        self.moved();
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
    /// return the rows to draw. Wrapping keeps the last column free, so
    /// the cursor after a full row's last character is still on screen.
    pub fn visible(&mut self, width: usize, height: usize) -> Vec<String> {
        let (width, height) = (width.max(1), height.max(1));
        if self.wraps() {
            self.width = (width - 1).max(1);
            let rows = self.rows(self.width);
            let (i, c) = locate(&rows, self.row, self.col);
            if i < self.top {
                self.top = i;
            } else if i >= self.top + height {
                self.top = i + 1 - height;
            }
            self.screen = (i - self.top, c);
            return rows
                .iter()
                .skip(self.top)
                .take(height)
                .map(|&(r, s, e)| self.lines[r][s..e].iter().collect())
                .collect();
        }
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
        if self.wraps() {
            return self.screen;
        }
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
    fn a_single_line_editor_keeps_one_line_and_scrolls_sideways() {
        let mut e = Editor::new(true);
        e.insert_str("first\nsecond");
        e.newline();
        assert_eq!(e.text(), "first second");
        let v = e.visible(4, 1);
        assert_eq!(v, vec!["ond".to_string()]);
        assert_eq!(e.cursor_on_screen(), (0, 3));
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_every_word_is_drawn() {
        let mut e = Editor::with_text("aaa bbb ccc\nd", false);
        e.to_start();
        // width 8: 7 columns of text, the last kept for the cursor
        let v = e.visible(8, 5);
        assert_eq!(v, vec!["aaa bbb ", "ccc", "d"]); // the space hangs
        let mut long = Editor::with_text("abcdefghij", false);
        assert_eq!(long.visible(5, 5), vec!["abcd", "efgh", "ij"]);
        let mut exact = Editor::with_text("exactly8 exactly8", false);
        assert_eq!(exact.visible(9, 5), vec!["exactly8 ", "exactly8"]);
    }

    #[test]
    fn the_cursor_is_on_screen_at_every_position_and_right_visits_each_once() {
        let text = "one two three four five six seven\n\nexactly8 exactly8\nx";
        for width in [2, 5, 8, 9, 13, 40] {
            let mut e = Editor::with_text(text, false);
            e.to_start();
            let mut seen = std::collections::HashSet::new();
            loop {
                let v = e.visible(width, 3);
                let (y, x) = e.cursor_on_screen();
                assert!(
                    x < width && y < 3 && y < v.len(),
                    "width {width} at {:?}",
                    e.cursor()
                );
                assert!(
                    seen.insert(e.cursor()),
                    "width {width}: {:?} twice",
                    e.cursor()
                );
                let before = e.cursor();
                e.right();
                if e.cursor() == before {
                    break;
                }
            }
            let positions: usize = text.split('\n').map(|l| l.chars().count() + 1).sum();
            assert_eq!(seen.len(), positions, "width {width}");
        }
    }

    #[test]
    fn up_and_down_move_by_drawn_rows_and_keep_their_column() {
        let mut e = Editor::with_text("aaaa bbbb cc dddd", false);
        e.to_start();
        e.visible(6, 5); // rows: "aaaa ", "bbbb ", "cc ", "dddd"
        e.right();
        e.right();
        e.right(); // column 3 of row 0
        e.down();
        assert_eq!(e.cursor(), (0, 8));
        e.down(); // "cc " has no column 3 short of the break: its end
        assert_eq!(e.cursor(), (0, 12));
        e.down(); // back to column 3 on "dddd"
        assert_eq!(e.cursor(), (0, 16));
        e.up();
        e.up();
        assert_eq!(e.cursor(), (0, 8));
    }

    #[test]
    fn words_move_and_erase_like_readline() {
        let mut e = Editor::with_text("one  two three", false);
        e.word_left();
        assert_eq!(e.cursor(), (0, 9));
        e.word_left();
        assert_eq!(e.cursor(), (0, 5));
        e.word_right();
        assert_eq!(e.cursor(), (0, 8));
        e.delete_word_back();
        assert_eq!(e.text(), "one   three");
        e.kill_to_end();
        assert_eq!(e.text(), "one  ");
        e.kill_to_start();
        assert_eq!((e.text().as_str(), e.cursor()), ("", (0, 0)));
    }

    #[test]
    fn undo_takes_back_a_word_a_run_of_erasing_or_a_paste_at_a_time() {
        let mut e = Editor::new(false);
        for c in "hello world".chars() {
            e.insert_char(c);
        }
        e.backspace();
        e.backspace();
        assert_eq!(e.text(), "hello wor");
        e.undo(); // the two backspaces
        assert_eq!(e.text(), "hello world");
        e.undo(); // "world"
        assert_eq!(e.text(), "hello ");
        e.insert_str("pasted\ntext");
        e.kill_to_start();
        assert_eq!(e.text(), "hello pasted\n");
        e.undo();
        e.undo(); // the paste
        assert_eq!(e.text(), "hello ");
        e.undo();
        assert_eq!(e.text(), "");
        assert!(!e.undo());
        e.set_text("loaded");
        assert!(!e.undo(), "loading starts a fresh history");
    }
}
