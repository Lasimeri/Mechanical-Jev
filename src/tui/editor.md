# tui/editor.rs: text editing

A buffer of lines of characters and a cursor, with what the keys of a
terminal map to: insert, newline, backspace and delete (joining lines at
their ends), the four arrows, home, end, page. `visible(width, height)`
scrolls so the cursor stays inside the view and returns what to draw;
`cursor_on_screen` says where to put the terminal's cursor. A single-line
editor (a question's id, its instructions, a file path) ignores newlines
and turns a pasted one into a space. No terminal here, so it is tested
directly: joining and splitting lines, the single-line rule, the view
following the cursor.
