# tui/model.rs: the draft request and the answer lines

What the TUI edits: the state as text and each question as a `QDraft`
(id, kind, instructions, and options as text, one per line: a Choice's
`key` or `key: description`, a Score's levels lowest first, a Noul's
optional `true: what yes means` and `false: what no means`, both or
neither). `Draft::to_request` builds the wire `Request`; every question is
checked by `protocol::parse_questions`, the client's own limits, so a bad
one is reported before any round trip. `Draft::from_request` goes the
other way (loading a file or one of TypeSafe's published examples).

TypeSafe's instructions, descriptions, levels and states may be JSON
structures, not just text (its "structured instructions" example). So
`structured` reads a field that parses as a JSON object or array as that
structure and anything else as trimmed text; `state_value` does the same
for the state but keeps text as written. Loading shows a structured
instruction pretty-printed and a structured description or level compact,
on its one line. A Choice key cannot contain `:` (the first `:` ends it).
`to_saved` and `from_saved` are the draft as kept between runs: every
field as typed, so a draft that does not validate yet survives a restart.
`summary` gives a question's instructions as one line for the list: the
first line of text, a structured instruction's `question` field, else the
structure compact.

What the TUI shows: `answer_lines` turns one answer into lines of label,
probability and note: a Noul's p(yes); each option of a Choice in the
question's order, the chosen one marked, then the confidence; each level
of a Score with its probability, the level nearest the expected score
marked, then the score and its confidence. With Jev's published answer to
the same question (an example from TypeSafe's documentation) each line
carries Jev's number beside the local one. `filled` sizes a bar.

Tested: a draft through the wire form and back, a JSON state kept as
structure, a bad Score caught before sending, a Choice's lines with Jev's
beside them, and every published request (`evidence::all`) through the
editor and back unchanged (it caught the Noul's criteria and the
structured fields, both lost before).
