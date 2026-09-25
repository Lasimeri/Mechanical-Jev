# tui/term.rs: the terminal, themed after seaof.glass

Raw mode and the alternate screen for as long as a `Term` lives, put back
on drop and on a panic of the thread that opened it (a hook restores the
terminal before the panic message prints, so a crash never leaves a
broken shell). A panic on any other thread (a TUI job) leaves the screen
alone, since drawing goes on, and is kept for `take_worker_panic` instead
of printed over the frame.

A `Frame` is the whole screen as rows of styled spans over a row colour,
drawn in one go inside a synchronized update, so a redraw does not
flicker. A span may carry its own background (`on`), for a highlight
narrower than the row. `set` cuts a row to the frame's width, so what a
`Frame` holds is what is drawn (`row_width`, `row_text` let tests check
it); each row is padded to the width in its own colour.

The palette is seaof.glass's, from the `:root` of its `index.html`
(Lasimeri/lasimeri.github.io, served at seaof.glass):

| name | colour | the site's | used for |
| --- | --- | --- | --- |
| `BG` | `#0a0a0f` | `--bg` | everything |
| `SURFACE` | `#12121a` | `--surface` (a hovered row) | the selected row |
| `BORDER` | `#1e1e2e` | `--border` | rules, the empty part of a bar |
| `TEXT` | `#c4945a` | `--text`, `--accent` | text, the filled part of a bar |
| `DIM` | `#8a6a3e` | `--text-dim` | secondary text, section labels |
| `ACCENT_DIM` | `#7a5c38` | `--accent-dim` | an unchosen option's bar |

Truecolor escapes; `crossterm` for raw mode, keys, bracketed paste and the
escape sequences, and no widget library (by the user's direction, not
ratatui).

`set` also draws every control character as one cell (a space for a tab,
`\r` or `\n`, `�` for the rest): a loaded file's tab, a log's carriage
return or an escape sequence inside an error would otherwise move the
terminal's cursor or change its colours and tear the frame.

Colours (`Colors`, read when the terminal opens): truecolor by default;
`MJEV_COLOR=256` maps each to its nearest xterm 256-colour index
(`ansi256`: the 6x6x6 cube or the grey ramp, whichever is closer; tested
on the palette) for a terminal without truecolor; `NO_COLOR` (or
`MJEV_COLOR=none`) draws none, the selected row in reverse video. Not
detected: `COLORTERM` is often missing over ssh and in tmux where
truecolor works, and guessing would quietly downgrade the palette. The
window's title is set to `mjev`.
