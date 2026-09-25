# tui/app.rs: the app

What is on screen and what each key does, with nothing drawn here
([`view.rs`](view.md) draws). Screens: home, ask, the question form,
examples, server, help. `key` takes a key event, `paste` a bracketed paste
(into the focused editor), `tick` the messages from jobs and the health
check when it is due. `run` is `mjev tui`'s loop: it draws when something
changed, waits up to 100 ms for a key, and ends on `Ctrl+Q` or `Ctrl+C`.
It refuses to start without a terminal on stdin and stdout.

## Jobs

Anything that waits on the network or on `xks` runs on a worker thread
and reports over a channel, so the screen never stops: asking (a
local server that does not answer is started first, `phi::ensure_as`, a
minute or so on the 35B; the job's label says so), starting (`phi::start_as`),
stopping (`phi::stop_as`, which gives the cards back), the models and the
health check. One of asking, starting and stopping at a time; the others
are refused with "busy". `xks`'s own output goes to a log, not the
screen (`phi::Say::Log`), and a failure's last lines show on the server
screen. A worker that panics becomes an error message, not a torn screen
(`term::take_worker_panic`).

The health check (`GET /health`, 2 s) runs every 5 s while no job runs,
for a server on this machine only: a remote one is asked directly and
shown as remote.

## Jev beside the local answer

A published example keeps its request and Jev's answers (`loaded`).
`jev_for` gives Jev's answer to a question only while that question and
the state are exactly the example's (compared in wire form), so a number
never stands beside a question it did not answer. The same rule marks a
local answer "changed since it was asked" once its question or the state
is edited (`answer_for`).

## Tests

With no terminal: home to ask to a saved Choice (the keys a user presses,
the request that results); a bad Score kept in the form with the error
shown; Jev's number shown for the unchanged example, gone after a state
edit, back after undoing it; every screen rendered at 20x5, 40x10, 80x24
and 200x60 with every row inside the width and the cursor on screen (run
in a debug build too, where arithmetic overflow panics).

## On quitting

The server keeps running, as it does after any `mjev` command. When it
was up at the last check, a line on exit (after the terminal is restored)
says so and that `mjev stop` ends it: on the cards site it holds 4.7 GiB
of each card's memory until then.

## Kept, undone, confirmed

- **The draft** is kept between runs in `$XDG_STATE_HOME/mjev/draft.json`
  (else `~/.local/state/mjev/draft.json`): the state and every question
  as typed, whether it validates yet or not (`Draft::to_saved`), with the
  example it came from, so Jev's numbers come back too. Written
  atomically (a temporary file, then a rename) on each saved question,
  each ask, on quitting, and every 5 s while it changes (a closed window
  or a kill loses at most those seconds of typing). `mjev tui` always
  opens with it (`open_with`); a `--file` loads over it like `l` does, so
  the kept draft is one `u` away. A run writes only what it changed
  (`keep_if_changed`): quitting an idle second TUI leaves the draft
  another one kept. The tests keep it nowhere (`draft_file` is `None`).
- **Undo:** `u` puts the draft back as it was before the last delete,
  move (`Alt+↑` `↓`), saved question, load or clear, up to 50 back, with
  the example it came from; the answers stay and show as changed where
  they no longer fit. So loading an example over a draft loses nothing.
  `n` twice (within 3 s) starts a new, empty draft, the focus left on
  the questions so the offered `u` works; `Ctrl+Q` twice quits
  while a job runs (a start carries on without the TUI).
- **The answer:** `r` writes the last response as JSON, as `w` writes
  the request. The answer before it is kept (`previous_for`), so a
  re-asked question shows what each probability was.
- **Messages:** an info message clears after 8 s; an error stays until
  the next message.
- **A start's progress:** while a start runs, the status row shows the
  last line of the server's own log (`phi::serve_log`), read from its
  tail every half second, a line rewritten with `\r` as its latest text,
  and only once the log is newer than the start (before that it is the
  previous run's). "The start" is when the job began, less a second: xks
  writes its first lines before the TUI reads the message that a start
  is under way, and a file's time comes from the kernel's coarse clock,
  a few milliseconds behind; both hid the line until a test and a live
  35B start caught them. xks says `loading NAME (N GB)` before the load,
  the line a start mostly shows.
- **Paths:** the file prompt takes `~/` for the home directory, and `Tab`
  completes as far as the matching entries agree (a directory gets its
  `/`), listing them when more than one matches; hidden entries only
  after a typed `.`.
- **The form:** `Esc` closes it at once when nothing changed; over
  changes it takes a second `Esc`, and says `Ctrl+S` saves them.
- **Long errors:** one of more than one line, or longer than 100
  characters, shows in full on the server screen; its first line stays
  on the status row.
