# Mechanical Jev

System One questions from Rust: a client library, the `mjev` command line
and an evaluation harness. You send a **state** and typed **questions**;
the answers come back typed, with probabilities, never as generated text:

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
cargo build --release
make query          # starts Intel Phi Jev if it is down, asks examples/query.json
./target/release/mjev query --state '$ git status' --noul 'ro=Is this command read-only?'
make eval           # the long real sessions, scored
make stop           # stop the server, release the cards
```

The first request starts the server (`xks serve --detach` from Intel Phi
Jev, which loads the model and puts the cards to work: about 45 s); after
that a question takes seconds.

## Commands

| command | does |
| --- | --- |
| `mjev query --file req.json` | one request (or `--state` with `--noul/--choice/--score`, or stdin) |
| `mjev eval cases.jsonl [--rows R] [--limit N]` | a labelled case file: accuracy, Brier, ECE, coverage at 5 percent error, latency |
| `mjev corroborate A B` | two recorded runs compared question by question |
| `mjev models` | what the server serves |
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
| `MJEV_XKS` | `~/Intel Phi Jev/target/release/xks` | Intel Phi Jev's binary |
| `MJEV_AUTOSTART` | `1` | start the local server when it is down |
| `TYPESAFE_DEFAULT_MODEL` | `jev-latest` | the model named in requests |
| `TYPESAFE_API_KEY` | none | a bearer token, for a server that wants one |

The variable names are the official TypeSafe SDKs', so the same settings
drive those SDKs against the same server.

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
