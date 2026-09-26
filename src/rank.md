# rank.rs: candidates ranked against a query

`mjev rank` asks one Noul per candidate and sorts by its probability of
yes, surest first: TypeSafe's re-ranking cookbook
(docs.typesafe.ai, `cookbooks/rerank_typesafe.md`), whose question is a
yes/no with `true` and `false` criteria and whose score is the noul. Use:
a shortlist from keyword or vector search re-ranked, retrieved passages
filtered before they reach a model, the line of a document that answers
a question ([`docs/uses.md`](../docs/uses.md)).

## Shape

The query is the state (`{"query": ...}`); each candidate is in its own
question, `c1`, `c2`, ... by its place in the input, worded
`INSTRUCTIONS` ("Does the candidate answer `query`?", naming the state's
field as the jev-1.13 jaggedness notes advise against indirection) with
the candidate after it, and the criteria `TRUE` and `FALSE`. On Intel Phi
Jev's `xks` the state is read once per request and every question forks
from it, so a candidate costs its own tokens only. `--instructions`
words the question differently.

Requests are cut at `CHUNK` (30) candidates, two rounds of `xks`'s 15
forks, or `CHUNK_CHARS` (24,000) characters of candidate text, which
keeps a request well under the token limits of both `xks` and the
hosted Jev. One primitive throughout: the jaggedness notes warn that a
Noul's and a Choice's probabilities do not compare, so a ranking never
mixes them. Equal scores keep input order.

## Tests

`cargo test rank::`: chunks by count and by text, the state and the
question as sent; every request one the server's checks accept.

Driven 2026-09-26 against the 2B at 127.0.0.1:8095: the ten rows of the
README's command table ranked against "How do I stop the server and give
the cards their memory back?" in 6.7 s, the `mjev serve` / `mjev stop`
row first at 0.68.
