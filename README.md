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
make build          # mjev
make query          # starts Intel Phi Jev if it is down, asks examples/query.json
./target/release/mjev query --state '$ git status' --noul 'ro=Is this command read-only?'
make eval           # the long real sessions, scored
make closeness      # Jev's published questions asked locally, compared with Jev's answers
make stop           # stop the server, release the cards
make tokenizers     # once: the nine pinned tokenizer.json files the fits read (sha256-checked)
make evidence       # the reverse engineering's fits, offline
make check          # docs, format, lint, build, tests
```

The server is [Intel Phi Jev](https://github.com/Lasimeri/Intel-Phi-Jev),
cloned and built (`make build`) next to this checkout; see
[The repositories](#the-repositories). The first request starts it
(`xks serve --detach`, which loads the model and puts the cards to work:
about 45 s); after that a question takes seconds.

## Use it

```sh
make install        # a link at ~/.local/bin/mjev (PREFIX= to change); make uninstall removes it
mjev                # alone, in a terminal: the TUI (also: mjev tui [--file request.json], make tui)
```

A terminal interface, themed after seaof.glass, for using Jev without
writing JSON:

1. **ask** (`a` on home): write the state (text, or a JSON object), `Tab`
   to the questions, `a` to add one. The form takes the kind (`←` `→`:
   noul, choice, score), an id, the instructions, and the options, one per
   line (choice: `key` or `key: description`; score: levels, lowest
   first; noul: optionally `true: ...` and `false: ...`). `Ctrl+S` saves,
   checked against TypeSafe's limits.
2. `F5` asks. When Intel Phi Jev's server is down it is started first
   (a minute or so for the 35B); the header shows what runs.
3. Each answer shows as bars under its question: every option's or
   level's probability, the chosen one in full copper, the confidence.
4. **examples** (`e`): TypeSafe's published requests. One loads into ask
   with Jev's published answer beside each question. After `F5`, Jev's
   number stands next to the local one, for as long as neither the
   question nor the state is edited.
5. **server** (`s`): state, subject, models; `s` starts, `x` stops and
   gives the cards back. While the server starts, the status row shows
   the line its log is on.
6. On the questions: `l` and `w` load and write request files (which
   `mjev query --file` takes too; `Tab` completes the path), `r` writes
   the last answer, `u` undoes a delete, move, save, load or clear,
   `Alt+↑` `↓` moves a question, `n` twice starts a new draft. Re-asked,
   each probability that moved says what it was.

The draft is kept between runs (`$XDG_STATE_HOME/mjev/draft.json`, else
`~/.local/state/mjev/draft.json`), written every few seconds as it
changes, a question that does not validate yet included. `F1` lists every key. On a first run with no Intel Phi Jev
built, home says where to build it. Colours are truecolor; `NO_COLOR` turns
them off (the selected row in reverse video) and `MJEV_COLOR=256` maps
them to the 256-colour palette for a terminal without truecolor. The plan and its stages are in
[docs/tui.md](docs/tui.md).

## Commands

| command | does |
| --- | --- |
| `mjev query --file req.json [--bars]` | one request (or `--state` with `--noul/--choice/--score`, or stdin); the response as JSON, or with `--bars` as the TUI draws it |
| `mjev eval cases.jsonl [--rows R] [--limit N]` | a labelled case file: accuracy, Brier, ECE, coverage at 5 percent error, latency |
| `mjev corroborate A B` | two recorded runs compared question by question |
| `mjev models` | what the server serves |
| `mjev tui [--file req.json]` | the terminal interface (see [Use it](#use-it)) |
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
