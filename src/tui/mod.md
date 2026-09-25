# tui/mod.rs: the terminal interface

`mjev tui`, a way to use Jev on this machine without writing JSON: write
a state, add typed questions, ask, and read every answer as bars, with
the local server started when it is down and stopped (the cards released)
from the same screen. Themed after seaof.glass (`term.md`).

| module | what |
| --- | --- |
| [`editor`](editor.md) | text editing |
| [`model`](model.md) | the draft request, to and from the wire; answers as lines |
| [`term`](term.md) | the terminal, frames, the palette |
| [`view`](view.md) | each screen drawn |
| [`app`](app.md) | screens, keys, jobs, the loop |

The plan, its stages and the keys: [`docs/tui.md`](../../docs/tui.md).
