# evidence: what Jev has been seen to do

Everything the reverse engineering ([`docs/reverse-engineering.md`](../docs/reverse-engineering.md))
stands on is here, and nothing else: TypeSafe's own published examples,
each kept whole with the page it was published on.

| file | what |
| --- | --- |
| [`published_pairs.json`](published_pairs.json) | the thirteen complete request and response pairs printed in TypeSafe's documentation, with their `usage` counts, in the documentation's own key order (the order is evidence: the input fit is 2.0 tokens rms in it and 2.5 in alphabetical order) |
| [`invariants.json`](invariants.json) | the two examples of the documentation's page on jaggedness, with the note that says what each shows |

The rules this archive keeps:

- Only what was published. Nothing here came from calling Jev or any other
  API; nothing is paraphrased, rounded or completed.
- Each item carries its `source`, the page it was taken from, so it can be
  checked against the original.
- Nothing is corrected. Where the published numbers disagree with each
  other (finding 12's output counts), the disagreement is the evidence.

`src/evidence.rs` reads these files into a case file and rows
(`mjev evidence`), and `tools/jevre` fits them (`make evidence`).

## A nod to The Eye

This directory is an archive before it is anything else, so a nod to the
admin and mod team of the Discord server of [The Eye](https://the-eye.eu/),
"a non-profit, community driven platform dedicated to the archiving and
long-term preservation of any and all digital heritage, obscura and
ideas", whose staff there have helped with "anything and everything from
sourcing content to delivery of our larger datasets" (2018). Their FAQ,
after Archive Team, gives the reason a directory like this one exists: a
publisher's pages can change or go without an announcement, and "your
data is never totally safe". TypeSafe's documentation is live and will be
revised; what the reverse engineering read is kept here as it was
published, with where it was published. "We are digital librarians", they
say; this is a very small shelf.
