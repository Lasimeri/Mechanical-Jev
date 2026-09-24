# Mechanical Jev

TypeSafe's **Jev** from Rust: a client library, the `mjev` command line,
and an evaluation harness. Jev is the first "System One" model. You send a
**state** and typed **questions** and get typed answers with probabilities,
never generated text:

| question | answer |
| --- | --- |
| Noul | the probability a yes/no statement is true |
| Choice | one option of up to 255, every option's probability, a confidence |
| Score | an ordered rubric of 2 to 10 levels: the expected level, every level's probability, a confidence |

Jev runs only on TypeSafe's servers. This repository is the client side and
nothing else: no local model, no inference.

## Start

```sh
cargo build --release
echo 'TYPESAFE_API_KEY=your-key' >> mjev.local.conf      # from console.typesafe.ai; not tracked
make query                                               # examples/query.json
./target/release/mjev query --state '$ git status' --noul 'ro=Is this command read-only?'
make eval                                                # the long real sessions, scored
```

## Commands

| command | does |
| --- | --- |
| `mjev query --file req.json` | one request (or `--state` with `--noul/--choice/--score`, or stdin) |
| `mjev eval cases.jsonl [--rows R] [--limit N]` | a labelled case file: accuracy, Brier, ECE, coverage at 5 percent error, latency |
| `mjev corroborate A B` | two recorded runs compared question by question (model versions, days) |
| `mjev models` | the models your key can use |

## Library

```rust
use mechanical_jev::{client::Client, protocol::RequestBuilder};

let req = RequestBuilder::new("Help! My payouts have been failing for 3 days.")
    .noul("is_urgent", "Does this convey urgency?")
    .choice("department", "Which team should handle this?",
            &[("billing", "Payments, invoicing, refunds"), ("technical", "Bugs, outages, integrations")])
    .build()?;
let (resp, took) = Client::from_env()?.system_one(&req)?;
let urgent = resp.noul("is_urgent");          // Some(0.95)
let (team, confidence) = resp.choice("department").unwrap();
```

The client checks TypeSafe's limits before sending (255 options, 2 to 10
levels) and retries 429 and 529 with exponential backoff, honouring
`Retry-After`, as TypeSafe's docs ask; 401 and 422 come back at once.

## Configuration

`mjev.conf` (tracked defaults), `mjev.local.conf` (this machine, not
tracked), `~/.config/mechanical-jev/mjev.conf`; the environment wins over
all of them. The variables are the official SDKs': `TYPESAFE_API_KEY`,
`TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`.

## Sources

The wire format and semantics follow [docs.typesafe.ai](https://docs.typesafe.ai/);
the confidence formulas are TypeSafe's own, from their MIT-licensed
`system-one-adapter`, with its test cases. The protocol and evaluation
code began in [jev-rs](https://github.com/yijunyu/jev-rs), by way of
[Intel-Phi-Jev](https://github.com/Lasimeri/Intel-Phi-Jev), which keeps the
local-model side. See [`NOTICE`](NOTICE). MIT or Apache-2.0.
