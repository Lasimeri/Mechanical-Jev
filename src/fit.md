# fit.rs: thresholds from your own answers

`mjev fit ROWS` reads what a server answered on labelled cases (`mjev
eval cases.jsonl --rows ROWS`) and says, per question, where its answers
can be trusted, then suggests a policy ([`policy.rs`](policy.md)) that
acts only there. It asks no model. docs.typesafe.ai's `confidence.md` says
the right thresholds depend on the domain and are to be tested on your
own data; the write-ups ([`docs/uses.md`](../docs/uses.md)) say the same
from experience (Aman Kumar: "you do not pick the drop threshold by
hand"). This is that step, so that `gate` and `label` do not run on
thresholds copied from an example.

## What it reports

Per question (`fit`, `Fitted`), from its rows:

- the accuracy (the argmax right; a Noul cut at 0.5);
- three bands, each with its share of the answers and how many of them
  were right: for a Noul P(yes) at or under 0.1, between, and at or over
  0.9; for a Choice or Score confidence under 0.5, between, and from 0.9
  (TypeSafe's example bands, `policy.rs`'s defaults);
- for a Noul, the drop line: half the lowest P(yes) any real yes was
  given (Aman Kumar's method), with the share of answers under it. On
  these rows no yes falls under it; below it a costlier check could be
  skipped. It is reported, not put in the policy: it is a filter's line,
  not an act, review or escalate one.

## What it suggests

For a Noul, `yes_at`: the lowest P(yes) (0.5 to 0.99, in steps of 0.01)
over which the answers are yes `--target` (0.95) of the time, and
`no_at` the same for no from below. For a Choice or Score, `act_at`: the
lowest confidence over which the answers are right that often, with the
share of answers it would act on; `floor` stays under it.

"That often" is measured at the lower end of what the answers show, not
their plain share: the one-sided 95 percent Wilson bound (`lower_bound`).
24 right of 25 is 0.96 seen and 0.84 at worst, so it does not set a
threshold at 0.95; about 60 right answers in a row do. The first run
without it (the 2B on Intel Phi Jev's 25 `dev_tasks`, 2026-09-26) put
`class` at `act_at` 0 because 24 of 25 were right: a threshold that acts
on everything, from 25 answers. Under `MIN_FIT` (10) answers a question
keeps the defaults outright, and under 30 the report says the result is
a starting point: set a threshold on part of the data and check it on the
rest. A question whose answers never reach the target keeps the default
and is flagged to check by hand.

`--out FILE` writes the suggested policy (only what is set: a written
`Rule` leaves out empty fields), checked by the policy's own rules;
`--json` prints the report as JSON.

## Tests

`cargo test fit::`: a Noul's drop line, bands and thresholds on 220
artificial readings; a lucky 24 of 25 not setting a threshold; a Choice's
bar on the first grid step past its highest wrong confidence; a question
never right enough keeping the default; too few answers keeping the
defaults; a written policy leaving out what is not set.

Driven 2026-09-26: `mjev eval` of the 2B at 127.0.0.1:8095 on Intel Phi
Jev's `examples/dev_tasks.jsonl` (25 real commands, 72.9 s, accuracy
0.64), then `mjev fit`: `class` 96 percent right, `rerun_safe` 60,
`verbosity` 36 and never confident; every threshold left at its default,
each flagged, and `label` with that policy escalated all eight commands
it was given (the `verbosity` answers are never sure).
