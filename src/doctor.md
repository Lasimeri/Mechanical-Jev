# doctor.rs: the family's setup check

`mjev doctor` (`make doctor`) is where the whole family is checked from,
and `make setup` (`mjev doctor --fix`) where it is set up. It looks at
what asking a question needs, in order, prints one line per finding
(`ok`, `note`, or `MISS` with its fix under it), and exits 0 when a
question can be asked, 1 when not:

| what | how | blocks when |
| --- | --- | --- |
| mjev | `PATH` resolved and canonicalised against this binary | never |
| PATH | whether `PREFIX/bin` is one of `PATH`'s directories | never |
| questions | `TYPESAFE_BASE_URL`: this machine (Intel Phi Jev) or hosted; for hosted, whether `TYPESAFE_API_KEY` is set (never its value) | hosted without a key |
| xks build | Intel Phi Jev's `xks` as [`phi.rs`](phi.md) finds it (`MJEV_XKS`, PATH, a checkout next to this one or in `$HOME`) | not built |
| (xks doctor) | `xks doctor [--fix] --prefix P`, its text relayed as it is and its exit code taken: the llama.cpp build, the subject, Intel-Phi-AVX512 and its payload, the stack's `phi`, the cards, memory, a server (Intel Phi Jev's `src/doctor.md`) | it exits 1 |
| server now | one `GET /health`, 2 s (`Client::health`) | never: down, the first question starts it (a note when `MJEV_AUTOSTART=0`) |

A hosted `TYPESAFE_BASE_URL` stops after the key: nothing local is
needed, and nothing is asked of the hosted service.

It starts nothing: `Doctor` is dispatched before `phi::ensure`, which
would start the server for any other command, and neither doctor
prepares a site, starts a worker or reaches a card over ssh.

`--fix` does what is a build or a link, never a card, a download or
`sudo`:

- `mjev` linked into `PREFIX/bin` (`--prefix`, default `~/.local`, the
  Makefile's `PREFIX`); `xks doctor --fix` links `xks` there and builds the
  payload when it is missing;
- `xks` built (`make build-x86` in its checkout) when it is not: first
  checking for the llama.cpp build it links (`LLAMA_BUILD_DIR`, else
  `LLAMA_CPP_DIR/build-native/bin`, else `~/llama.cpp/build-native/bin`),
  which a cargo error would only hint at, and on a failure the last lines
  of the build's output.

A link is created when absent and replaced when it is dangling or links
another build; a file in its place is never replaced (`place_link`, tested
with each case). `make install` refuses the same way.

Measured 2026-09-25 on this machine: as it stood it exits 0 with notes
(neither command on PATH yet, the stack's `phictl` and `phitop` debug
builds, no server running); `MJEV_XKS` at a missing build, and a hosted
URL without a key, exit 1; a scratch checkout whose build fails shows the
build's last lines; `LLAMA_BUILD_DIR` missing is said before any build;
`make setup PREFIX=scratch` linked both commands and ended ready. No xks
process or listener appeared in any of them.
