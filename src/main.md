# main.rs: the `mjev` command line

Loads defaults ([`config.rs`](config.md)), then `query`, `eval`,
`corroborate`, `models`, `reconstruct`, `serve` or `stop`. Every command
that talks to the server first makes sure it answers, starting Intel Phi
Jev's server on this machine when it is down ([`phi.rs`](phi.md));
`corroborate` and `reconstruct` work offline. `reconstruct` prints what Jev
most likely does with a request ([`reconstruction.rs`](reconstruction.md)).
`eval` adds the model, the case count and the wall time to its metrics.
