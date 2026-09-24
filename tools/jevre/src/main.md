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
DeepSeek V3). They are not in the repository.

- `input`: fits `input_tokens` as a fixed preamble, the state, and each
  question's JSON plus a wrapper by type, for every tokenizer and
  serialization; best fits first.
- `output`: the published `output_tokens` against each response's structure,
  and an additive fit.
- `probs`: TypeSafe's confidence formulas against every published answer,
  and for sample counts N whether counts k out of N could round to every
  published number (probabilities, confidence, a Score's expected level).
