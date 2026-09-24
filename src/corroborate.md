# corroborate.rs: two runs, question by question

Joins two `eval --rows` files on (case, question id), aligns their options
by key (two runs may list options in different orders), and reports the
largest and mean absolute difference of the option probabilities, how
often the answer agrees, and each run's accuracy.

`--temperature` adds, per question kind, the temperature that brings B's
probabilities closest to A's (`p^(1/t)` renormalised, searched from 0.2 to
about 20): above one means A reads softer than B, below one sharper. Against
Jev's published answers it says how much more or less committed Jev is than
the server under test.

Uses: Jev's published answers against Intel Phi Jev (`make closeness`), a
model version against the previous one, the same cases on two days.
