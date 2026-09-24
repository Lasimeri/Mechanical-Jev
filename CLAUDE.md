# Mechanical-Jev: notes for an agent working here

Read `CONTRIBUTING.md` first; it is the authority. The non-obvious rules:

- What this is: the asking side of Jev (TypeSafe System One) in Rust:
  protocol, client, confidence, eval, corroborate, the `mjev` CLI, and
  Jev reverse engineered from its documentation (`docs/reverse-engineering.md`).
  No inference. The server is Intel-Phi-Jev's `xks`, started on demand and
  found by `MJEV_XKS`, `xks` on PATH, a checkout next to this one or in
  `$HOME`, under either name.
- The spec is docs.typesafe.ai and TypeSafe's MIT `system-one-adapter`;
  unofficial Jev sites (jevai.net and the like) are not. The reverse
  engineering uses the published documentation only: never call Jev or any
  other API for it.
- Rust only. No Python or JavaScript, ever.
- Sibling `.md` per code file, same change. No em or en dashes anywhere.
- The server needs no key; a key, if ever set, lives in `mjev.local.conf`
  (ignored) and is never printed.
- Test data is TypeSafe's own documented examples, real text, or obviously
  artificial. `make check` before committing, push after.
