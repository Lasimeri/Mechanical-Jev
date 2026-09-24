# reconstruction.rs: Jev's server, inferred

The request pipeline TypeSafe's documentation implies, written as code. Every
step is an inference from published numbers, with its evidence and
confidence in [`docs/reverse-engineering.md`](../docs/reverse-engineering.md).

- `document`: a string state verbatim, a structure as JSON indented by two
  spaces.
- `compile`: one `Branch` per question: its compact JSON without the id (what
  the model reads), its outcomes (option keys, level numbers, or `true`), a
  Score's legend.
- `budget`: under a tokenizer's count, the inferred billed input (263-token
  preamble, document, branches with their wrappers) and TypeSafe's two
  limits: 32k for the state and the longest question, 64k for the request.
- `assemble`: a branch's distribution as a Jev answer, with TypeSafe's
  confidence formulas, the Score's expected level, and two-decimal rounding
  as Python's `round` does it.

The tests read `evidence/published_pairs.json`: rebuilding all sixteen
published Choice and Score answers from their probabilities gives back
their choice, confidence, score and legend; no branch carries anything but
`type`, `instructions` and `criteria`; an oversized branch is refused.
