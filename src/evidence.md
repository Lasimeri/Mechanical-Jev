# evidence.rs: Jev's published answers as a yardstick

`mjev evidence` writes two files from the published pairs
([`evidence/published_pairs.json`](../evidence/published_pairs.json)) and the
jaggedness page's two structural examples
([`evidence/invariants.json`](../evidence/invariants.json)):

- `target/evidence-cases.jsonl`: every request, its gold the answer Jev
  published (the argmax of its probabilities; a Noul yes when 0.5 or more).
- `target/evidence-jev-rows.jsonl`: Jev's published probabilities as the
  rows `mjev eval` records.

`mjev eval` of the cases against any server, then `mjev corroborate` of the
two row files, says how closely that server answers like Jev: its accuracy
is its agreement with Jev's decisions. `make closeness` does all of it
against Intel Phi Jev. 15 cases, 28 questions: 24 in the 13 pairs, 4 in the
two examples. The evidence keeps the docs' key order (options in the order
TypeSafe listed them), which matters for position bias and for the token
fits.
