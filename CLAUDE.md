# Mechanical Jev: notes for an agent working here

- System One questions to Intel Phi Jev, asking side only, in Rust: protocol, client, phi, confidence,
  eval, corroborate, the `mjev` CLI. No inference; the server (a local model
  on the host and Phi cards) is github.com/Lasimeri/Intel-Phi-Jev, started on demand.
- The spec is docs.typesafe.ai and TypeSafe's MIT `system-one-adapter`;
  unofficial Jev sites (jevai.net and the like) are not.
- The server needs no key; a key, if ever set, lives in `mjev.local.conf` (ignored).
- Sibling `.md` per code file, no dashes, `make check` green, push after.
