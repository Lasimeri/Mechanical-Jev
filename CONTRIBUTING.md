# Contributing

These rules exist so that every number here can be re-run with one
command and every inference traced to what TypeSafe published. The
sections are the same in every repository of the family (see
[The family](#the-family)); what differs is said where it applies.

## Languages

- **Rust** for everything, `tools/jevre` included.
- **Shell and make** only for `scripts/check-docs.sh` and the `Makefile`,
  which check the tree, call into `mjev` and `jevre`, or fetch the pinned
  tokenizers (`make tokenizers`).
- **Never Python or JavaScript** for anything here.
- The asking side only: nothing here runs a model; Intel-Phi-Jev does.

## Documentation

- Every code file has a sibling `.md` with the same stem; a behaviour
  change updates it in the same commit.
- No em or en dash characters anywhere, commit messages included.
- Relative links between Markdown files must resolve. A file that lives in
  a sibling repository is linked on GitHub, never named as if it were here.
- `scripts/check-docs.sh` enforces the sibling, dash and link rules
  (`make docs-check`, the first step of `make check`).

## Measurements

- The reverse engineering uses TypeSafe's published documentation only
  (`evidence/`, each item with its source page): Jev is never called, and
  no other API either. The fits run offline (`make evidence`) over nine
  public tokenizers pinned in `tools/jevre/TOKENIZERS` (`make tokenizers`
  fetches them once, checked by sha256).
- A closeness number cites the command that produced it (`make closeness`,
  with its `LAYOUT` and `PERMUTATIONS`).
- Test data is TypeSafe's own documented examples, real text, or obviously
  artificial. Never invented people, companies, tickets or accounts.

## The family

| repository | what | finds its dependency by |
| --- | --- | --- |
| [Intel-Phi-3120A](https://github.com/Lasimeri/Intel-Phi-3120A) | the cards' software stack: daemon, kernel, boot, storage, the `phi` CLI | (none) |
| [Intel-Phi-AVX512](https://github.com/Lasimeri/Intel-Phi-AVX512) | the cards as an AVX-512 co-processor: phi512, the card worker, the `libggml_phi.so` backend | `PHI_STACK_ROOT`, `phi` on PATH, a checkout next to it, `$HOME` |
| [Intel-Phi-Jev](https://github.com/Lasimeri/Intel-Phi-Jev) | `xks`, a local Jev (System One) whose subject runs on the host and the cards | `PHI_AVX512_ROOT`, a checkout next to it, `$HOME` |
| [Mechanical-Jev](https://github.com/Lasimeri/Mechanical-Jev) (this one) | `mjev`, the asking side of Jev, and Jev reverse engineered from its docs | `MJEV_XKS`, `xks` on PATH, a checkout next to this one, `$HOME` |

- A dependency is found in that order, as a checkout under its GitHub
  clone's name (`Intel-Phi-Jev`) or the spaced one (`Intel Phi Jev`)
  ([`src/phi.md`](src/phi.md)). Nothing of a sibling is copied into
  another (the one exception is between Intel-Phi-3120A and
  Intel-Phi-AVX512: the `knc-mvex` library, kept identical by the latter's
  `make check`).
- What this repository consumes from Intel-Phi-Jev: `xks serve --detach
  --bind`, `xks stop`, `target/release/xks` (where a checkout's binary is
  looked for), `$XDG_RUNTIME_DIR/xks/serve.log` (read as a start's
  progress), `/health`, `/v1/models`, the System One wire format, and
  `xks doctor [--fix] --prefix P` (`mjev doctor` relays its text and takes
  its exit code: 0 ready, 1 not).

## Git

- One subject line that says what changed (a leading `Area:` is fine), then
  the why. `make check` before every commit, push after.
- Never commit a key: it lives in `mjev.local.conf`, which git ignores.
- MIT or Apache-2.0 ([`LICENSE-MIT`](LICENSE-MIT),
  [`LICENSE-APACHE`](LICENSE-APACHE)); the portions from jev-rs keep their
  authors' notice ([`NOTICE`](NOTICE)).
