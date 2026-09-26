# Mechanical Jev

System One questions from Rust: a client library, the `mjev` command line,
a terminal interface and an evaluation harness. You send a **state** and
typed **questions**; the answers come back typed, with probabilities,
never as generated text:

| question | answer |
| --- | --- |
| Noul | the probability a yes/no statement is true |
| Choice | one option of up to 255, every option's probability, a confidence |
| Score | an ordered rubric of 2 to 10 levels: the expected level, every level's probability, a confidence |

The server is [Intel Phi Jev](https://github.com/Lasimeri/Intel-Phi-Jev)
on this machine: a local model on this host and its Xeon Phi cards,
answering the System One wire format (`POST /v1/systemone`, the format of
TypeSafe's Jev). This repository holds no model and runs no inference; it
asks.

## Start

```sh
# the server, cloned next to this checkout (see The repositories)
git clone https://github.com/Lasimeri/Intel-Phi-Jev ../Intel-Phi-Jev

make setup          # build and link mjev and xks into ~/.local/bin, build what is missing,
                    # and check everything down to the cards (PREFIX= to change)
mjev                # the TUI; try: e, Enter, F5
```

`make setup` is `mjev doctor --fix`: every step a question needs, in
order (mjev, where questions go, Intel Phi Jev's `xks`, its llama.cpp
build and subject, Intel-Phi-AVX512's payload, the cards, a server), one
line each, and for anything missing the command that fixes it. It builds
and links what it can and never boots a card, downloads a model or runs
`sudo`: those it names. `mjev doctor` checks without changing anything,
any time something stops working ([`src/doctor.md`](src/doctor.md)).

The first question starts the server (`xks serve --detach`: it loads the
model and puts the cards to work, under a minute); after that a question
takes seconds. Quitting leaves it running, as every command does:
`mjev stop` ends it and gives the cards their memory back.

From the command line, and for development:

```sh
mjev query --file examples/query.json --bars   # one request, answers as bars (JSON without --bars)
mjev query --state '$ git status' --noul 'ro=Is this command read-only?'
make eval           # the long real sessions, scored
make closeness      # Jev's published questions asked locally, compared with Jev's answers
make stop           # stop the server, release the cards
make tokenizers     # once: the nine pinned tokenizer.json files the fits read (sha256-checked)
make evidence       # the reverse engineering's fits, offline
make check          # docs, format, lint, build, tests
```

## Use it

`mjev` alone, in a terminal, opens the TUI (also `mjev tui [--file
request.json]` or `make tui`): Jev without writing JSON, themed after
seaof.glass.

**A first look.** `e` lists TypeSafe's published requests; `Enter` loads
one, with Jev's published answer under each question; `F5` asks the local
server (starting it when it is down; the header shows what runs, the
status row what the server's log says). Each answer is a bar per option
or level, the chosen one marked `•`, then the confidence, with Jev's
number beside ours while neither the question nor the state is edited.

**Your own request.** On **ask** (`a` on home), write the state (text, or
a JSON object), `Tab` to the questions, `a` to add one: the kind (`←` `→`:
noul, choice, score), an id, the instructions, and the options, one per
line (choice: `key` or `key: description`; score: levels, lowest first;
noul: optionally `true: ...` and `false: ...`). The form checks the
question as it is typed; `Ctrl+S` saves. `F5` asks; asked again after an
edit, each probability that moved says what it was.

**Around it.**

- On the questions: `Enter` edits, `d` deletes, `Alt+↑` `↓` moves, `u`
  undoes a delete, move, save, load or clear, `n` twice starts afresh;
  `l` and `w` load and write request files (the same files `mjev query
  --file` takes; `Tab` completes a path), `r` writes the last answer.
- Every text field has the readline keys (`Ctrl+A` `E` `K` `U` `W`, word
  moves) and `Ctrl+Z` to undo typing.
- **server** (`s` on home): state, subject, models; `s` starts, `x` stops
  and gives the cards back. With no Intel Phi Jev built, home and this
  screen say how to build it.
- The draft is kept between runs, written every few seconds as it
  changes (`$XDG_STATE_HOME/mjev/draft.json`, else
  `~/.local/state/mjev/draft.json`).
- Colours are truecolor; `NO_COLOR` turns them off (the selected row in
  reverse video), `MJEV_COLOR=256` maps them to the 256-colour palette.
- `F1` lists every key. The plan and its stages: [docs/tui.md](docs/tui.md).

## Commands

| command | does |
| --- | --- |
| `mjev query --file req.json [--bars]` | one request (or `--state` with `--noul/--choice/--score`, or stdin); the response as JSON, or with `--bars` as the TUI draws it |
| `mjev eval cases.jsonl [--rows R] [--limit N]` | a labelled case file: accuracy, Brier, ECE, coverage at 5 percent error, latency |
| `mjev corroborate A B` | two recorded runs compared question by question |
| `mjev models` | what the server serves |
| `mjev tui [--file req.json]` | the terminal interface (see [Use it](#use-it)) |
| `mjev doctor [--fix]` | the family's setup check: what asking needs, down to the cards, and the fix for what is missing; `--fix` (`make setup`) builds and links; exit 0 ready, 1 not |
| `mjev serve` / `mjev stop` | start the server / stop it and release the cards |
| `mjev reconstruct --file req.json` | what Jev most likely does with a request (offline): the document and each question's branch as the model reads it |
| `mjev evidence` | Jev's published answers as a case file and rows, to measure a server against (`make closeness`) |

## Library

```rust
use mechanical_jev::{client::Client, phi, protocol::RequestBuilder};

let req = RequestBuilder::new("$ cargo clean && rm -rf ~/.cargo/registry")
    .noul("outside", "Does the command delete anything outside the project?")
    .choice("class", "What kind of command is this?",
            &[("build", "compiles or bundles code"), ("package", "installs or removes dependencies")])
    .build()?;
let client = Client::from_env();
phi::ensure(&client)?;                        // start Intel Phi Jev if it is down
let (resp, took) = client.system_one(&req)?;
let outside = resp.noul("outside");           // Some(0.94)
let (class, confidence) = resp.choice("class").unwrap();
```

The client checks the limits before sending (255 options, 2 to 10 levels)
and retries 429 and 529 with exponential backoff, honouring `Retry-After`.

## Configuration

`mjev.conf` (tracked defaults), `mjev.local.conf` (this machine, not
tracked), `~/.config/mechanical-jev/mjev.conf`; the environment wins over
all of them.

| key | default | meaning |
| --- | --- | --- |
| `TYPESAFE_BASE_URL` | `http://127.0.0.1:8090` | the server |
| `MJEV_XKS` | found (below) | Intel Phi Jev's binary, when it is somewhere else |
| `MJEV_AUTOSTART` | `1` | start the local server when it is down |
| `TYPESAFE_DEFAULT_MODEL` | `jev-latest` | the model named in requests |
| `TYPESAFE_API_KEY` | none | a bearer token, for a server that wants one |

The variable names are the official TypeSafe SDKs', so the same settings
drive those SDKs against the same server.

## The repositories

| repository | what | how it is found |
| --- | --- | --- |
| [Intel-Phi-3120A](https://github.com/Lasimeri/Intel-Phi-3120A) | the cards' software stack: boots them, serves their memory, the `phi` command | by Intel-Phi-AVX512 |
| [Intel-Phi-AVX512](https://github.com/Lasimeri/Intel-Phi-AVX512) | the cards as an AVX-512 co-processor, whose ggml backend runs the model's multiplies | by Intel-Phi-Jev |
| [Intel-Phi-Jev](https://github.com/Lasimeri/Intel-Phi-Jev) | `xks`, the server: a local Jev on this host and the cards | `MJEV_XKS`, else `xks` on PATH, else `target/release/xks` in a checkout next to this one, else in `$HOME` |
| Mechanical-Jev (this one) | `mjev`, the asking side, and Jev reverse engineered | |

Cloned side by side, the repositories find each other without
configuration, under each one's clone name (`Intel-Phi-Jev`) or the
spaced one (`Intel Phi Jev`) ([`src/phi.md`](src/phi.md)). What does need
setting is Intel Phi Jev's: the model and llama.cpp paths in its
`xks.conf`, and for the cards, the stack's `phi` command with a card up.
[`CONTRIBUTING.md`](CONTRIBUTING.md) has the rules they share.

## Sources

The wire format and semantics follow [docs.typesafe.ai](https://docs.typesafe.ai/);
the confidence formulas are TypeSafe's own, from their MIT-licensed
`system-one-adapter`, with its test cases. The protocol and evaluation
code began in [jev-rs](https://github.com/yijunyu/jev-rs), by way of
Intel Phi Jev. See [`NOTICE`](NOTICE). MIT or Apache-2.0.

## Jev, reverse engineered

[`docs/reverse-engineering.md`](docs/reverse-engineering.md) infers what
TypeSafe's Jev does inside from its published documentation alone, without
calling it: a fixed preamble of about 263 tokens, each question sent as its
compact JSON without the id, the state prefilled once and every question a
branch from it, a readout over the offered options (not counted from a
small number of samples), a separate Noul readout, and confidence by
TypeSafe's own formulas (all sixteen published answers reproduced). The
inferences are code in [`src/reconstruction.rs`](src/reconstruction.rs); the
data is [`evidence/published_pairs.json`](evidence/published_pairs.json);
the fits are [`tools/jevre`](tools/jevre/src/main.rs).

Measured against Jev's own published answers (`make closeness`), Intel Phi
Jev's local subject with lettered options, the docs' option order and three
rotations averaged makes the same decision as Jev on all 28 published
questions; the details, and why Jev's own input layout does worse on an
untrained model, are in the report's last section.

## The foundation

The family's names stand on the Revelation to John (Intel Phi Jev's
README, Naming: "a wise man, which built his house upon a rock", Matthew
7:24). Here it is closer than a name. The book calls itself an
*apokalypsis*, an unveiling (1:1), and that is what
[`docs/reverse-engineering.md`](docs/reverse-engineering.md) does: Jev
unveiled from what TypeSafe chose to publish, without calling it. And the
evidence it stands on is kept by the book's last rule, "If any man shall
add unto these things ... if any man shall take away from the words of
the book" (22:18 to 19): nothing added to what was published, nothing
taken away ([`evidence/README.md`](evidence/README.md)).
