# tui/mod.rs: the terminal interface

`mjev tui`, a way to use Jev on this machine without writing JSON: write
a state, add typed questions, ask, and read every answer as bars, with
the local server started when it is down and stopped (the cards released)
from the same screen. Themed after seaof.glass (`term.md`).

Built in stages. Here and tested: `editor` (text editing), `model` (the
draft request and the answer lines), `term` (the terminal and the
palette). Next: the app loop that joins them, its screens, keys and
background jobs, as planned in [`docs/tui.md`](../../docs/tui.md).
