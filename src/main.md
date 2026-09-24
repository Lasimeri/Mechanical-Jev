# main.rs: the `mjev` command line

Loads defaults ([`config.rs`](config.md)), then `query`, `eval`,
`corroborate`, `models`, `reconstruct`, `evidence`, `serve` or `stop`. Every
command that talks to the server first makes sure it answers, starting
Intel Phi Jev's server on this machine when it is down ([`phi.rs`](phi.md)).
`corroborate`, `reconstruct` and `evidence` work offline: `reconstruct`
prints what Jev most likely does with a request
([`reconstruction.rs`](reconstruction.md)); `evidence` writes Jev's
published answers as cases and rows ([`evidence.rs`](evidence.md)).
`eval` adds the model, the case count and the wall time to its metrics.
