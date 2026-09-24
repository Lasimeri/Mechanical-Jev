# Mechanical Jev: notes for an agent working here

- TypeSafe's Jev, client side only, in Rust: protocol, client, confidence,
  eval, corroborate, the `mjev` CLI. No inference; the local-model side is
  github.com/Lasimeri/Intel-Phi-Jev.
- The spec is docs.typesafe.ai and TypeSafe's MIT `system-one-adapter`;
  unofficial Jev sites (jevai.net and the like) are not.
- `TYPESAFE_API_KEY` lives in `mjev.local.conf` (ignored). Never print it.
- Sibling `.md` per code file, no dashes, `make check` green, push after.
