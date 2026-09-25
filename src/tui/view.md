# tui/view.rs: drawing

Each screen as a [`term::Frame`](term.md), in seaof.glass's colours and
lowercase. Nothing here changes what the app holds but scroll positions.

- **home**: as the site opens: the name, the verse under it ("and before
  the throne there was a sea of glass like unto crystal", Revelation 4:6,
  dim and italic; left out when the terminal is short), a section rule
  (`── jev on the phi cards ──`), the rows `/ ask` `/ examples` `/ server`
  `/ help` `/ quit` with a dim description on the right and the selected
  row on the surface colour, then the server's address and state. When
  Intel Phi Jev is not built, how to build it (`phi::not_built`).
- **ask**: the state's editor (as tall as its text, within a third of
  the screen, so a short state leaves the room to the answers), then the
  questions, each with its answer under it: a `•` on the chosen option
  (visible without colour too), a label, a bar (`█` for the probability, `─` for the rest;
  copper for the chosen option, the dimmer accent for the others), the
  value, and Jev's number as a note; one label column for every answer,
  so all the bars start together. A Score's expected level is a line of
  its own with no bar. The selected question's rows are kept in view.
- **form**: kind, id, instructions, options, the focused field marked `›`;
  the options' hint follows the kind.
- **examples**: TypeSafe's published requests, one row each (ids, kinds,
  the state's first line), where the selected one is from and its note.
- **server**: address, state, subject, models, which `xks`, where the logs
  are, and the last lines `xks` wrote.
- **help**: the keys, scrolling with `↑` `↓` when they outnumber the rows
  (the rule says which way there is more); a test keeps every row inside
  80 columns.

Every screen has a header (where you are on the left, the server or the
running job with a spinner on the right) and a footer (the status or a
file prompt, then the keys). Below 40x10 only "the terminal is too small"
is drawn. Widths count characters; wide characters (CJK, emoji) are taken
as one column.

The keys row takes `(key, what)` pairs in order of use and adds them while
they fit, keeping `f1 help` last, so a narrow terminal drops the least
used first. A question's row sizes its id column from the longest id (up
to 24, cut with `…`) and shows its instructions as one line
(`model::summary`: a structured instruction's `question` field).

The form checks the question as it is typed, the way saving will ("ready:
3 options", or "not yet:" and why). The examples screen shows the
selected request in full under the list: where it is from, its note and
each question with its kind and instructions. While a start runs, the
status row shows the server log's latest line.
After an answer the questions' rule says how long it took and how many
tokens the server read (the header names the model).
