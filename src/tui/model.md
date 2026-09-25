# tui/model.rs: the draft request and the answer lines

What the TUI edits: the state as text and each question as a `QDraft`
(id, kind, instructions, and options as text: a Choice's `key` or
`key: description` per line, a Score's levels one per line, lowest first).
`Draft::to_request` builds the wire `Request`: a state that parses as a
JSON object or array goes as that structure, anything else as text; every
question is checked by `protocol::parse_questions`, the client's own
limits, so a bad one is reported before any round trip.
`Draft::from_request` goes the other way (loading a file or one of
TypeSafe's published examples).

What the TUI shows: `answer_lines` turns one answer into lines of label,
probability and note: a Noul's p(yes); each option of a Choice in the
question's order, the chosen one marked, then the confidence; each level
of a Score with its probability, the level nearest the expected score
marked, then the score and its confidence. With Jev's published answer to
the same question (an example from TypeSafe's documentation) each line
carries Jev's number beside the local one. `filled` sizes a bar.

Tested: a draft through the wire form and back, a JSON state kept as
structure, a bad Score caught before sending, a Choice's lines with Jev's
beside them.
