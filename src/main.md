# main.rs: the `mjev` command line

Loads defaults ([`config.rs`](config.md)), then one of `query`, `eval`,
`corroborate`, `models`. `corroborate` works offline on recorded rows; the
others need `TYPESAFE_API_KEY`, and without it they stop with a message
saying where to put it. `eval` adds the model, the case count and the wall
time to its metrics, so a record says what produced it.
