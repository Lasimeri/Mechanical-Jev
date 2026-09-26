# main.rs: the `mjev` command line

Loads defaults ([`config.rs`](config.md)), then `query`, `eval`,
`corroborate`, `models`, `reconstruct`, `evidence`, `serve`, `stop` or
`tui`; `mjev` alone is `tui` when stdin and stdout are a terminal, and
elsewhere the usage error it always was (exit 2). Every command that
talks to the server first makes sure it
answers, starting Intel Phi Jev's server on this machine when it is down
([`phi.rs`](phi.md)); `tui` ([`tui/app.rs`](tui/app.md)) starts nothing up
front and starts the server when it is asked to (`--file` opens a request
file).
`corroborate`, `reconstruct` and `evidence` work offline: `reconstruct`
prints what Jev most likely does with a request
([`reconstruction.rs`](reconstruction.md)); `evidence` writes Jev's
published answers as cases and rows ([`evidence.rs`](evidence.md)).
`eval` adds the model, the case count and the wall time to its metrics.

`query --bars` prints the answers as the TUI draws them (each question,
its options or levels with bars, a `•` on the chosen one) instead of the
JSON response ([`tui/model.rs`](tui/model.md), `answers_text`).

`--choice id=instructions|options` and `--score id=instructions|levels`:
the last `|` starts the options, so instructions may contain `|`; options
or levels are separated by `;` when there is one (so a description may
contain commas), else by `,`. An empty option, an option without a key,
and a key given twice are errors.

`doctor` is dispatched right after the client is built and before
`phi::ensure`, so it never starts the server; it prints its report
([`doctor.md`](doctor.md)) and exits 0 ready, 1 not. `--prefix` takes
`~/` for the home directory (default `~/.local`).

## gate

`gate` asks as `query` does (the same flags, `Ask`) and judges the
answers with a policy ([`policy.rs`](policy.md)): one line per answer
(its value, decision and outcome, and the threshold it was measured
against), composites with their dimensions, then the overall outcome;
`--json` prints the verdict whole. The exit code is the outcome: 0 act,
10 review, 11 escalate; 1 stays an error (the server unreachable, a bad
file, a policy naming a question the request does not ask) and 2 clap's
usage error, so a shell `if mjev gate ...` never mistakes a failure for a
decision. The request and the policy are read and checked before the
server can be started, as `query`'s request now is too: a mistake in
either costs a moment, not a model load.

Driven 2026-09-26 against the 2B at 127.0.0.1:8095, on TypeSafe's own
support-ticket example (the Stripe integration message of their
primitives page) with its three questions: exit 11, the frustration Score
escalated at confidence 0.25; with a policy file, the speculative Noul
reported and ignored, a composite of 0.64 acting; a policy typo exit 1
before any call; an unreachable server exit 1.

## label and rank

`label` ([`label.rs`](label.md)) and `rank` ([`rank.rs`](rank.md)) read
their input (a file, or stdin, refused at once when stdin is a terminal
and no file is named), their questions and their policy before the
server can be started. `label` writes one JSON line per state (to
`--out` or stdout) and exits 1 when any state failed; `rank` prints
`P  index  candidate` lines (or `--json`), `--top N` the best N.

`fit` ([`fit.rs`](fit.md)) is offline like `corroborate`: dispatched
before the client exists, it reads rows, prints the report (or `--json`)
and writes the suggested policy to `--out`; `--target` must be 0.5 to 1.

`guard` ([`guard.rs`](guard.md)) is dispatched right after `doctor`, before
`phi::ensure`: a hook never starts a server. It reads the hook's JSON on
stdin, prints the hook's decision or nothing, and always exits 0 (a
failure's reason goes to stderr).
