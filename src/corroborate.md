# corroborate.rs: two runs, question by question

Joins two `eval --rows` files on (case, question id): the largest and mean
absolute difference of the option probabilities, how often the answer
agrees, each run's accuracy. Its uses: a new model version against the old
(`TYPESAFE_DEFAULT_MODEL`), the same cases on two days, or Jev against any
other system that writes the same rows.
