# label.rs: the same questions over many states

`mjev label` runs one question set over every state of an input, one
request per state, and judges each response with a policy
([`policy.rs`](policy.md)): the bulk shape people use Jev in (a pile of
expense reports, an inbox, pages from a crawl; [`docs/uses.md`](../docs/uses.md)).
It is `patterns/intent-routing.md` and `patterns/fan-out.md` of
docs.typesafe.ai applied line by line: the questions travel together,
the routing is the outcome each line carries.

## Input

One state per non-empty line (`items`): a JSON object with a `state` is
that state, and its `id` (any JSON value) is copied to the output; any
other JSON value is the state; a line that is not JSON is a text state.
`--text` reads every line as text, so a line like `"quoted"` or `42`
stays what was written. Blank lines are skipped, and the output carries
the input line number, so a result can be traced to its line.

The questions come from `--questions FILE` (the questions map, or a whole
request whose `questions` are used; `questions_of`) and from
`--noul/--choice/--score` as in `query`, merged; they are checked against
TypeSafe's limits, and the policy against them, before the server can be
started.

## Output

One JSON line per state (`run`): `line`, `id` when given, then the
verdict (`outcome`, `answers` with each one's value, decision, outcome
and the threshold behind it, `composites`), or `error` for a state whose
request failed; the rest go on. On stderr the count of each outcome, and
`label: N of M` as it goes when stderr is a terminal. The exit code is 1
when any state failed (the lines say which), else 0: the outcomes are
data here, not the exit code (`gate` is the one-state command whose exit
code is the outcome).

## Tests

`cargo test label`: states as objects, values and text, and as text
only; questions bare and in a request; an unreachable server recorded per
line while the rest go on, with progress for each.

Driven 2026-09-26 against the 2B at 127.0.0.1:8095, over the states of
TypeSafe's first six published example requests (`evidence/`, read, not
written): 6 states in 8.5 s, act 0, review 5, escalate 1 (the 2B is
rarely sure of this Noul), one line each.
