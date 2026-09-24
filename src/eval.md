# eval.rs: scoring Jev on labelled cases

A case file is JSONL (`#` lines are comments): `state`, `questions`, and
`gold` (question id to the expected option key, level number, or
`"yes"`/`"no"`). `run` asks Jev each case and keeps every answer as a
`Row`: the option keys in question order and Jev's probability of each
(Noul: P(yes), 1 - P(yes)). A call that fails for transport or rate
reasons counts the case as failed and moves on; an invalid case or a bad
key stops the run.

`metrics`: accuracy, Brier, top-label ECE over 10 bins, mean confidence,
coverage at 5 percent error (the largest share of questions answerable,
most confident first, with the error at or under 5 percent), latency
percentiles (a request's time shared out among its questions), accuracy
per question kind.

## Failures and labels (2026-09-24)

- A case whose response lacks an answer, carries one of another type, or
  lacks what its type carries (a Noul's `noul`, every option's
  probability) is a failed case, counted and skipped, like a transport
  failure. It used to end the whole run (a missing answer) or be scored as
  a guess (a missing probability read as 0, a missing Noul as 0.5).
- A gold label names an option: a key (a Score's level number is its
  key; a number that is a key is that key), `true`/`false` for a Noul, else
  a number read as an index, and only an index inside the options. A
  1-based level is an error, not a silent miss.
- Coverage at 5 percent error is what a confidence threshold accepts, so
  the sorted rows are cut only where the confidence changes: rows of equal
  confidence are accepted or refused together.
