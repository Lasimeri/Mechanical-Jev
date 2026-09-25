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

`--choice id=instructions|options` and `--score id=instructions|levels`:
the last `|` starts the options, so instructions may contain `|`; options
or levels are separated by `;` when there is one (so a description may
contain commas), else by `,`. An empty option, an option without a key,
and a key given twice are errors.
