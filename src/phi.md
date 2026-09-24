# phi.rs: Intel Phi Jev's server on demand

`ensure` asks `GET /health`; if the server is on this machine
(`127.0.0.1` or `localhost`) and does not answer, it runs Intel Phi Jev's
`xks serve --detach --bind host:port`, which loads the subject and site
from that repository's `xks.conf`, puts the Phi cards to work, and returns
once the server answers (about 45 s for the 35B). A remote server that does
not answer is an error, never started. `MJEV_AUTOSTART=0` turns the
starting off. `stop` runs `xks stop`, which ends the server and releases
the cards' huge pages.

Measured 2026-09-24, from a stopped server: `mjev query` of
examples/query.json took 47 s end to end (start, weight upload to the
cards, the first request 19.5 s); the next single question took 1.8 s,
served on the cards site (devices Phi and CPU).

## Finding xks

In order, the first that exists:

1. `MJEV_XKS`, a path to the binary.
2. `xks` on `PATH`.
3. `target/release/xks` in a checkout next to this one, named
   `Intel-Phi-Jev` (a `git clone`) or `Intel Phi Jev`.
4. The same two names in `$HOME`.

When no checkout has `xks` built, the first checkout found (it has
`Cargo.toml`) is the one the "not built" error names, so a fresh clone is
told where to run `make build-x86`.

The same order finds every sibling in this family of repositories
(Intel Phi Jev finds Intel-Phi-AVX512, which finds Intel-Phi-3120A).
`find_sibling` is the search, tested against a temporary tree holding
each name. "Next to this one" is the directory this checkout was built
in (`CARGO_MANIFEST_DIR`), not where the binary is run from.
