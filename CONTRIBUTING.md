# Contributing

- Rust only. No Python, no JavaScript.
- The asking side only: nothing here runs a model; Intel Phi Jev does.
- Every code file has a sibling `.md`; a behaviour change updates it in the
  same commit. No em or en dashes.
- Test data is real text or obviously artificial; never invented people,
  companies, tickets or accounts. The tests use TypeSafe's own documented
  examples.
- `make check` before a commit. Never commit a key: it lives in
  `mjev.local.conf`, which git ignores.
