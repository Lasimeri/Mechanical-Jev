# tui/editor.rs: text editing

A buffer of lines of characters and a cursor, with what the keys of a
terminal map to: insert, newline, backspace and delete (joining lines at
their ends), the four arrows, home, end, page. `visible(width, height)`
scrolls so the cursor stays inside the view and returns what to draw;
`cursor_on_screen` says where to put the terminal's cursor. `to_start`
puts the cursor and the view at the top, so a loaded state reads from its
beginning.

A multi-line editor (the state, a question's instructions and options)
wraps each line to the view: after the last space that fits, else
mid-word for a word longer than the row. The view's last column is kept
free, so the cursor after a full row's last character is still on screen,
and a space right after a full row hangs there instead of starting the
next row by itself. Up and down move by the rows as drawn (the width is
the last view's), aiming for the same column across a run of them, so a
short row passed on the way does not pull the cursor left. Home and end
stay on the line, not the drawn row. `rows_at(width)` is how many rows
the text takes in a view that wide, which sizes the state's box.

The readline keys a terminal user expects: `word_left` and `word_right`
(stopping at spaces), `delete_word_back`, `kill_to_end` and
`kill_to_start`, `to_start` and `to_end`. `undo` takes back a step: a run
of typing up to a space, a run of erasing, a paste, a newline or a kill
each one step, a move ending the step in progress; the last 100 are kept,
and `set_text` (a load) starts a fresh history.

A single-line editor (a question's id, a file path) ignores newlines,
turns a pasted one into a space, and scrolls sideways.

No terminal here, so it is tested directly: joining and splitting lines;
the single-line rule and its sideways view; wrapping at spaces, mid-word
and with a hanging space; at six widths, the cursor on screen at every
position with `right` visiting each position exactly once; up and down by
drawn rows keeping their column; the word moves and kills; undo by
word, by run of erasing and by paste, and a load clearing it.
