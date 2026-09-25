# The TUI: plan

`mjev tui`: Jev on this machine, usable without writing JSON. Rust, no
widget library (not ratatui, by the user's direction): `crossterm` for raw
mode, keys and escapes, the layout in `src/tui/`. Themed after seaof.glass.

## Stages

| stage | what | state |
| --- | --- | --- |
| 1 | `src/tui/editor.rs`: multi-line and single-line text editing | done, tested |
| 1 | `src/tui/model.rs`: the draft request to and from the wire form; answers as lines with bars; Jev's published answer beside ours | done, tested |
| 1 | `src/tui/term.rs`: raw mode, the alternate screen, restore on panic, frames of styled rows, seaof.glass's palette | done |
| 2 | `src/tui/app.rs`: the screens, keys, background jobs, the health check | next |
| 2 | `mjev tui` in `src/main.rs`, `make tui` | next |
| 3 | the README's "Use it" section; a first-run note when no Intel Phi Jev is built | after |

## The look (seaof.glass)

From `index.html` of Lasimeri/lasimeri.github.io: copper `#c4945a` on
`#0a0a0f`, secondary text `#8a6a3e`, rules `#1e1e2e`, the hovered row
`#12121a`. Lowercase everywhere. The home screen opens as the site does:
the name, then the verse ("and before the throne there was a sea of glass
like unto crystal", Revelation 4:6) in the dim colour, italic. Section
labels between rules (`── ask ──`; the site's own rules are em dashes,
which this project does not use, so box drawing). Menu rows as the site's
project rows: `/ ask` on the left, a dim description on the right, the
selected row on the surface colour. Bars: filled cells `█` in copper, the
rest `─` in the border colour.

## Screens

- **home**: the name and the verse; `── jev on the phi cards ──` with rows
  `/ ask`, `/ examples`, `/ server`, `/ help`, `/ quit`; below, the
  server line (address, up or down, the subject from `/health`).
- **ask**: the state (a multi-line editor) above, the questions below;
  after an answer, each question's lines under it (bars, the chosen option
  marked, confidence, Jev's number beside ours for an example). `Tab`
  moves between state and questions.
- **question form**: kind (`←` `→`: noul, choice, score), id, instructions,
  options (choice: `key` or `key: description` per line; score: a level per
  line, lowest first; noul: none). Checked by the client's own limits on
  save.
- **examples**: TypeSafe's published requests (`evidence/`), each loading
  into ask with Jev's published answers kept for comparison.
- **server**: address, health, subject, models; start (`xks serve
  --detach`), stop (`xks stop`, which gives the cards back).
- **help**: the keys.

## Keys

| where | key | does |
| --- | --- | --- |
| anywhere | `Ctrl+C`, `Ctrl+Q` | quit |
| anywhere | `F1` | help |
| anywhere but ask's editor | `Esc` | back |
| home | `↑` `↓` `Enter`, or `a` `e` `s` `h` `q` | choose |
| ask | `F5`, `Ctrl+S` | ask (starts the local server when it is down) |
| ask | `Tab` | state or questions |
| ask, questions | `↑` `↓`; `a` add; `Enter` or `e` edit; `d` delete | the list |
| ask, questions | `l` load a request file; `w` write one; `x` examples | files |
| form | `Tab`, `Shift+Tab` | next, previous field |
| form | `←` `→` on the kind | noul, choice, score |
| form | `Ctrl+S`, `F2` | save; `Esc` cancels |
| server | `s` start; `x` stop; `r` refresh | the server |

## How it runs

- One thread draws and reads keys (a 100 ms poll: the spinner turns, jobs
  report). Anything slow runs on a worker thread and reports by channel:
  asking (`phi::ensure`, then `Client::system_one`: a first ask starts
  the server, about a minute on the 35B), starting, stopping, the models.
- A health check (`GET /health`, 2 s timeout) every 5 s when nothing else
  is running; the status line says up or down and the subject.
- The terminal is restored on exit and on a panic (`term.rs`).

## Not in the first version

Running `eval` and `closeness` from the TUI (they take minutes and their
output is a report, better read in the terminal or the records); card
state beyond what `xks stop` does (Intel-Phi-AVX512's `phi-vpu.sh
config` could feed it later).
