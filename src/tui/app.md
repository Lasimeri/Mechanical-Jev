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
