# tools/jevre: the analysis behind the reverse engineering

A separate Cargo project, so its dependency on the `tokenizers` crate never
reaches `mjev`. It reads [`evidence/published_pairs.json`](../../../evidence/published_pairs.json)
and a directory of `tokenizer.json` files, and calls nothing.

    JEVRE_TOKENIZERS=/path/to/tokenizers cargo run --release -- input
    JEVRE_TOKENIZERS=/path/to/tokenizers cargo run --release -- output
    cargo run --release -- probs

`JEVRE_TOKENIZERS` holds one subdirectory per tokenizer, each with its
`tokenizer.json` (the tokenizer files of public models; the analysis used
o200k, gpt-oss, cl100k, Llama 3, Phi-4, Qwen3, Gemma 3, Mistral and
DeepSeek V3), by default `tools/jevre/tokenizers/`. They are not in the
repository: [`TOKENIZERS`](../TOKENIZERS) pins each one (repository,
revision, sha256 of the file the analysis read), and `make tokenizers`
fetches them with `hf download` at those revisions and checks the sums; it
is the only step that uses the network, and it downloads files, it calls
no model. `make evidence` runs `probs` always and `input` and `output`
when the tokenizers are there; without them `jevre` says so and exits 2
rather than panicking (before 2026-09-24 a fresh checkout's
`make evidence` stopped on a panic).

- `input`: fits `input_tokens` as a fixed preamble, the state, and each
  question's JSON plus a wrapper by type, for every tokenizer and
  serialization; best fits first.
- `output`: the published `output_tokens` against each response's structure,
  and an additive fit; then against the token count of text the response
  carries (the answers as compact or pretty JSON, without legends or
  types, the probability objects, the answer values, the option keys),
  `a + b * tokens` per tokenizer, best first. None comes near the
  structural fit (18.5 tokens rms at best, 1.84 for structure).
- `probs`: TypeSafe's confidence formulas against every published answer,
  and for sample counts N whether counts k out of N could round to every
  published number (probabilities, confidence, a Score's expected level).
