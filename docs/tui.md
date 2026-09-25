# The TUI: plan

`mjev tui`: Jev on this machine, usable without writing JSON. Rust, no
widget library (not ratatui, by the user's direction): `crossterm` for raw
mode, keys and escapes, the layout in `src/tui/`. Themed after seaof.glass.

## Stages

| stage | what | state |
| --- | --- | --- |
| 1 | `src/tui/editor.rs`: multi-line and single-line text editing | done, tested |
| 1 | `src/tui/model.rs`: the draft request to and from the wire form; answers as lines with bars; Jev's published answer beside ours | done, tested; every published request round-trips (Noul criteria and structured fields added in stage 2) |
| 1 | `src/tui/term.rs`: raw mode, the alternate screen, restore on panic, frames of styled rows, seaof.glass's palette | done; the panic hook restores only for the drawing thread (stage 2) |
| 2 | `src/tui/app.rs`: the screens, keys, background jobs, the health check | done, tested without a terminal and driven in tmux |
| 2 | `src/tui/view.rs`: each screen drawn | done; every screen fits 20x5 to 200x60 |
| 2 | `mjev tui [--file F]` in `src/main.rs`, `make tui` | done |
| 2 | `phi::{ensure_as, start_as, stop_as}` with `Say::Log`, `Client::health` | done; `xks`'s output in a log, not over the screen |
| 3 | the README's "Use it" section; a first-run note when no Intel Phi Jev is built | done |
| polish 1 | control characters drawn safely; ids, summaries and keys that fit; wrapping editors (75af1b5 and before) | done |
| polish 2 | the draft kept between runs, every 5 s while it changes, never lost to `--file` or an idle second TUI; `u` over deletes, moves, saves, loads and clears; the answer written; a start's progress (live on a 35B start); the form checked as typed; example details; `mjev` alone opens it; `make install`; `NO_COLOR` and `MJEV_COLOR=256` | done |
| polish 3 | readline keys and `Ctrl+Z` in every field; aligned bars, `•` on the choice, a Score's level on its own line; "was" after a re-ask; paths that complete and wrap whole; a changed form kept on one `Esc`; help that scrolls and fits 80 columns; the first run's build hint; `mjev query --bars` | done |
| next | asked with the server up on the cards, by a person at a real terminal | not yet |

## The look (seaof.glass)

From `index.html` of Lasimeri/lasimeri.github.io: copper `#c4945a` on
`#0a0a0f`, secondary text `#8a6a3e`, rules `#1e1e2e`, the hovered row
`#12121a`. Lowercase everywhere. The home screen opens as the site does:
the name, then the verse ("and before the throne there was a sea of glass
like unto crystal", Revelation 4:6) in the dim colour, italic (left out on
a short terminal). Section labels between rules (`── ask ──`; the site's
own rules are em dashes, which this project does not use, so box
drawing). Menu rows as the site's project rows: `/ ask` on the left, a dim
description on the right, the selected row on the surface colour. Bars:
filled cells `█`, copper for the chosen option and `#7a5c38` (the site's
`--accent-dim`) for the others, the rest `─` in the border colour.

## Screens

- **home**: the name and the verse; `── jev on the phi cards ──` with rows
  `/ ask`, `/ examples`, `/ server`, `/ help`, `/ quit`; below, the
  server line (address, up or down, the subject from `/health`), and how
  to build Intel Phi Jev when it is not built.
- **ask**: the state (a multi-line editor) above, the questions below;
  after an answer, each question's lines under it (bars, the chosen option
  marked, confidence, Jev's number beside ours for an unchanged example;
  "changed since it was asked" once the question or the state is edited).
  `Tab` moves between state and questions.
- **question form**: kind (noul, choice, score), id, instructions,
  options (choice: `key` or `key: description` per line; score: a level per
  line, lowest first; noul: optionally `true: ...` and `false: ...`).
  Checked by the client's own limits on save; an error keeps the form open.
- **examples**: TypeSafe's published requests (`evidence/`), each loading
  into ask with Jev's published answers kept for comparison.
- **server**: address, state, subject, models, which `xks`, the logs, the
  last lines `xks` wrote; start (`xks serve --detach`), stop (`xks stop`,
  which gives the cards back).
- **help**: the keys.

## Keys

| where | key | does |
| --- | --- | --- |
| anywhere | `Ctrl+C`, `Ctrl+Q` | quit (twice while a job runs); the draft is kept |
| anywhere | `F1` | help |
| anywhere | `Esc` | back (home from ask, from its editor too); a form or a file prompt: cancel |
| home | `↑` `↓` `Enter`, or `a` `e` `s` `h` `q` | choose |
| ask | `F5`, `Ctrl+S` | ask (starts the local server when it is down) |
| ask | `Tab` | state or questions |
| ask, state | keys, paste | edit; `Enter` is a new line |
| any text | `Ctrl+Z` | undo: a word typed, a run of erasing, a paste, a kill |
| any text | `Ctrl+A` `E` `K` `U` `W` (and `Ctrl+Backspace`) | home, end, erase to the end, to the start, the word before |
| any text | `Ctrl` or `Alt` with `←` `→` (`Alt+B`, `Alt+F`); `Ctrl+Home`, `Ctrl+End` | a word left, right; the text's start, end |
| ask, questions | `↑` `↓`; `a` add; `Enter` or `e` edit; `d` delete | the list |
| ask, questions | `l` load a request file; `w` write one; `r` write the last answer; `x` examples | files |
| ask, questions | `u` undo a delete, move, save, load or clear (50 back); `n` twice, a new draft; `Alt+↑` `↓` (or `Shift`) move a question | the draft |
| form | `Tab`, `Shift+Tab` | next, previous field |
| form, on the kind | `←` `→` or space; `n` `c` `s` | noul, choice, score |
| form | `Enter` on the kind or the id | next field |
| form | `Ctrl+S`, `F2` | save; `Esc` cancels (twice over changes) |
| a file prompt | `Tab`; `~/` | complete the path; the home directory |
| server | `s` start; `x` stop; `r` refresh | the server |

## How it runs

- One thread draws and reads keys (a 100 ms poll: the spinner turns, jobs
  report). Anything slow runs on a worker thread and reports by channel:
  asking (`phi::ensure_as`, then `Client::system_one`: a first ask starts
  the server, about a minute on the 35B), starting, stopping, the models.
  `xks`'s output goes to `$XDG_RUNTIME_DIR/mjev/xks.log`, not the screen.
- A health check (`GET /health`, 2 s timeout) every 5 s when nothing else
  is running; the header says up or down and the subject.
- The terminal is restored on exit and on a panic of the drawing thread; a
  job's panic becomes an error on the status row (`term.rs`).
- Quitting leaves the server running, as the command line does; when it
  is up, a line on exit says so and that `mjev stop` releases the cards.

## Not in the first version

Running `eval` and `closeness` from the TUI (they take minutes and their
output is a report, better read in the terminal or the records); card
state beyond what `xks stop` does (Intel-Phi-AVX512's `phi-vpu.sh
config` could feed it later).
