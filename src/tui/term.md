# tui/term.rs: the terminal, themed after seaof.glass

Raw mode and the alternate screen for as long as a `Term` lives, put back
on drop and on a panic (a hook restores the terminal before the panic
message prints, so a crash never leaves a broken shell). A `Frame` is the
whole screen as rows of styled spans over a row colour, drawn in one go
inside a synchronized update, so a redraw does not flicker. Text past the
right edge is cut; each row is padded to the width in its own colour.

The palette is seaof.glass's, from the `:root` of its `index.html`
(Lasimeri/lasimeri.github.io, served at seaof.glass):

| name | colour | the site's | used for |
| --- | --- | --- | --- |
| `BG` | `#0a0a0f` | `--bg` | everything |
| `SURFACE` | `#12121a` | `--surface` (a hovered row) | the selected row |
| `BORDER` | `#1e1e2e` | `--border` | rules, the empty part of a bar |
| `TEXT` | `#c4945a` | `--text`, `--accent` | text, the filled part of a bar |
| `DIM` | `#8a6a3e` | `--text-dim` | secondary text, section labels |
| `ACCENT_DIM` | `#7a5c38` | `--accent-dim` | notes (Jev's numbers) |

Truecolor escapes; `crossterm` for raw mode, keys, bracketed paste and the
escape sequences, and no widget library (by the user's direction, not
ratatui).
