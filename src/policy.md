# policy.rs: act, review or escalate

An answer says *what*; its probability or confidence says *whether to
act on it*. That second decision is the caller's, and it lives here, in
code and a policy file, never in the model. Every behaviour below is from
TypeSafe's documentation (docs.typesafe.ai), the page named beside it;
the unofficial sites and posts that describe people using Jev this way
([`docs/uses.md`](../docs/uses.md)) are evidence of use, not the spec.

## Outcomes

| outcome | meaning | `mjev gate` exits |
| --- | --- | --- |
| `act` | confident enough to act without anyone looking | 0 |
| `review` | a reasonable answer, not a sure one: confirm, or have it checked | 10 |
| `escalate` | unsure: do not act; a person or another system | 11 |

The three paths are `confidence.md`'s ("high: act automatically; medium:
proceed with caution; low: do not act"). The exit codes stay clear of 1
(an error: the server down, a timeout, a bad file) and 2 (clap's usage
error), so a failed call can never read as a decision in a shell `if`.
A verdict's outcome is the worst of its deciding answers and composites.

## Rules

- **Noul**: no confidence field (`primitives/noul.md`, `confidence.md`:
  "Noul answers don't carry one"); the probability is the certainty, so
  it is judged by its distance from 0.5: at or over `yes_at` (0.9) a
  confident yes, at or under `no_at` (0.1) a confident no, both `act`;
  between, `undecided` and `review` (the self-consistency cookbook for
  Nouls routes that middle to review, keeping the value visible).
- **Choice**: confidence under `floor` (0.5) `escalate`; at or over
  `act_at` (0.9) `act`; between, `review`. `act_at_option` sets a
  different bar for one option, because thresholds scale with risk: the
  voice banking example in `patterns/confidence-routing.md` (a 0.6
  floor, 0.85 to approve a transfer, 0.6 to show a balance) is a test.
- **Score**: the same bands on its confidence. `above` thresholds the
  score itself (the decision is `above` or `below` it); the jaggedness
  notes for jev-1.13 allow a threshold on the expectation and warn
  against reading a magnitude between levels, so nothing here does.
  Without `above` the decision names the nearest level and its legend.
- **ignore**: an answer asked speculatively (`patterns/fan-out.md`: ask
  everything in one call, let code ignore what turned out irrelevant) is
  reported and marked, and never decides the outcome, so an unused answer
  is never mistaken for a checked one.
- **composites** (`patterns/composite-scoring.md`): weights over answers
  made 0 to 1 (a Score's level over its top level, a Noul's probability),
  averaged by weight; with `act_above` (and `review_above`) it decides,
  without it it only ranks. The dimensions are kept in the verdict, raw,
  as counted and with their weight: a 0.82 alone cannot say which
  dimension made it. A Choice cannot be weighed (its options have no
  order), and saying so is an error.

The built-in thresholds (0.9, 0.1, 0.5, 0.9) are TypeSafe's worked
examples. `confidence.md` says plainly that the right values depend on
the domain: start conservative, test on your own data, adjust; `mjev
fit` ([`fit.rs`](fit.md)) suggests them from recorded answers and what
they should have been.

## A policy file

```json
{
  "defaults": {"floor": 0.6, "act_at": 0.8},
  "questions": {
    "intent": {"act_at_option": {"approve_transfer": 0.85}},
    "bug_severity": {"ignore": true},
    "frustration": {"above": 1.5}
  },
  "composites": {
    "senior_ic": {"weights": {"python": 0.4, "design": 0.4, "lead": 0.2},
                  "act_above": 0.7, "review_above": 0.4}
  }
}
```

Unknown fields are refused (a misspelt `yes` for `yes_at` would otherwise
be silently ignored), thresholds must be 0 to 1 and in order, weights
positive, and a policy naming a question the request does not ask is an
error, not a no-op.

## Tests

`cargo test policy`: a Noul at either end and between; the voice banking
Choice; a Score's threshold and named level; the worst answer deciding
and a speculative one not; TypeSafe's composite scoring arithmetic with
the dimensions kept; every mistake in a policy said before any answer;
exit codes apart from 1 and 2.
