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
