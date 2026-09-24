# confidence.rs: TypeSafe's formulas

As TypeSafe's MIT-licensed `system-one-adapter` computes them
(`_utils/confidence_metrics.py`), with its test cases as Rust tests:

- Choice: `(p_max - 1/n) / (1 - 1/n)`, the peak probability scaled from
  uniform to certainty (the docs' three-option demo, `(3 p_max - 1) / 2`,
  is this).
- Score: `1 - E|level - mode| / MAD(uniform)`, concentration around the
  modal level, so a near miss costs less than a far one.

Jev returns `confidence` itself; these reproduce it from `probabilities`
and give the same measure for any distribution (the evaluation uses them
for coverage).
