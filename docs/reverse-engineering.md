# Reverse engineering Jev from its documentation

What TypeSafe's Jev most likely does inside, inferred only from what
TypeSafe has published: its documentation at docs.typesafe.ai, its launch
post, and its MIT-licensed `system-one-adapter`. Jev was never called; no
model was run; the analysis is offline and repeatable
([`tools/jevre`](../tools/jevre/src/main.md)). The inferences are written as
code in [`src/reconstruction.rs`](../src/reconstruction.md).

## The evidence

TypeSafe's docs print thirteen complete request and response pairs with
`usage.input_tokens` and `usage.output_tokens`, sixteen Choice and Score
answers with rounded probabilities and confidence, and prose about limits,
latency and cost. The pairs are in
[`evidence/published_pairs.json`](../evidence/published_pairs.json), each
with its source page. Nine public tokenizers (the o200k family of GPT-4o and
gpt-oss, cl100k, Llama 3, Phi-4, Qwen3, Gemma 3, Mistral, DeepSeek V3) count
the text of each request.

## Findings

| # | finding | evidence | confidence |
| --- | --- | --- | --- |
| 1 | Every request carries a fixed hidden preamble of about **263 tokens** | the intercept of the fit below; 257 to 264 under every tokenizer | high |
| 2 | Each question reaches the model as its **compact JSON object** (`type`, `instructions`, `criteria`), never its id | 4 constants fit 13 pairs to 2.0 tokens rms, in the docs' own key order; pretty JSON or plain text fit far worse; the docs say ids are not sent | high |
| 3 | A structured state is written as **JSON indented by two spaces**, a string state verbatim | the best fits use it; the playground builds exactly this (`JSON.stringify(state, null, 2)`) | high |
| 4 | The state is prefilled **once** and every question is its own **branch** from it, all read in one pass | the limits: "32k tokens for state plus the longest question", "64k per request"; "Jev ingests the state once"; 13 questions cost 0.27 s against 0.21 s for one | high |
| 5 | A branch's answer is a probability distribution over the offered outcomes, read from the model's output distribution, **not counted from a small number of samples** | no sample count up to 128 (8, 16, 32, 64, 100, 128) can produce all sixteen published answers; 129 is the smallest that can | high for N at most 128; open between reading the distribution and 200 or more samples |
| 6 | **Noul is its own readout**, not a two-option Choice | the docs: the same question gives Noul 0.22 but Choice yes 0.01; a Noul and its negation sum to 1.19 | high |
| 7 | Confidence is exactly the `system-one-adapter` formulas: Choice `(p_max - 1/n) / (1 - 1/n)`, Score `1 - E|level - mode| / MAD(uniform)`; Score is the expected level | 16 of 16 published answers reproduced | high |
| 8 | Numbers are rounded to two decimals the way Python's `round` does (ties to even on the binary value) | the published 0.12 beside 0.88 (0.125 and 0.875 underneath, confidence 0.81) | medium |
| 9 | The model is a **small transformer**: about 3 to 10 B active parameters | about 11,100 state tokens read in at most 0.21 s including the network (53,000 tokens a second or more per request); $0.042 per million tokens; "transformer based" (press) | medium |
| 10 | The tokenizer is most likely the **o200k family** (GPT-4o, gpt-oss) | best fit (2.04 rms) ahead of cl100k and Llama 3 (2.40): a preference, not a proof | low |
| 11 | Trained with **RLCD**: a pretrained transformer post-trained with a reward that scores its probability distribution against outcomes (a proper scoring rule), on synthetic data | TypeSafe's AI primer and press; the scoring rule is the only reward that makes "probability 0.8 is right 80 percent of the time" the optimum | medium (the mechanism is inferred, the aim is stated) |
| 12 | `output_tokens` grows with Choice options (about 8 per option) and Nouls (about 18 each) but is not an additive function of structure (three Scores cost less than their parts) | the published counts | unexplained; they are not billed |

A candidate that fits every finding: an open-weight mixture of experts
with about 3.6 B active parameters and the o200k tokenizer (gpt-oss-20b is
one), post-trained with RLCD, served with prefix sharing and batched
branches. That is a hypothesis consistent with the evidence, not a
conclusion from it.

## The fit behind findings 1 to 3

`input_tokens = preamble + tokens(state) + sum over questions of
(tokens(question JSON) + wrapper by type)`, least squares over the thirteen
pairs (`jevre input`), best tokenizer:

| constant | tokens |
| --- | --- |
| preamble | 263.0 |
| wrapper per Noul | 6.1 |
| wrapper per Choice | 2.5 |
| wrapper per Score | 1.9 |
| residuals | +1 -3 -2 0 +2 -1 0 +4 +4 -2 0 -1 0 (2.0 rms) |

The Noul's larger wrapper matches finding 6: a Noul is framed differently
(a statement to judge true or false) from a Choice or a Score. The largest
residuals (+4) are the Choice whose options are JSON objects, where the
exact serialization inside the prompt is the least certain part, and the
two-Noul request. The fit was 2.5 tokens rms with the options in
alphabetical order and 2.0 in the docs' order: the question JSON keeps the
caller's key order.

## What the server does, as code

[`src/reconstruction.rs`](../src/reconstruction.md):

1. `compile`: the document (finding 3) and one branch per question, the
   question's compact JSON without its id (finding 2); the outcomes are the
   option keys, the level numbers, or `true` for a Noul (finding 6).
2. `budget`: preamble, document and the longest branch within 32k, all of
   it within 64k (finding 4); the billed input is preamble plus document
   plus every branch and its wrapper (finding 1).
3. `assemble`: a branch's distribution becomes the answer: argmax and
   Choice confidence, expected level and Score confidence, P(true) for a
   Noul, all rounded as Python rounds (findings 7 and 8). Its test rebuilds
   all sixteen published answers from their own probabilities.

`mjev reconstruct --file request.json` prints what the model most likely
reads for a request.

## Limits of this analysis

- Thirteen pairs. The structure fits well; the exact preamble text, the
  wrapper tokens and the base model are not recoverable from counts.
- Published examples may have been edited for the docs; one inconsistency
  (finding 12) suggests some were.
- Nothing here says how accurate or calibrated Jev is; that needs Jev.

## Refinement against a local subject

The inferences can be tested without Jev: build a local imitation with
[Intel Phi Jev](https://github.com/Lasimeri/Intel-Phi-Jev) (`xks`, the
Qwen3.8-35B-A3B on this host and its Xeon Phi cards), ask it Jev's own
published questions, and compare its answers with Jev's
(`make closeness`: `mjev evidence`, `mjev eval`, `mjev corroborate
--temperature`). The 13 published pairs and the 2 examples of the
jaggedness page give 28 questions; the gold is Jev's own answer.

| local configuration | same answer as Jev | mean largest probability difference |
| --- | --- | --- |
| lettered options, alphabetical order | 26 of 28 | 0.159 |
| lettered options, the docs' order | 27 of 28 | 0.120 |
| lettered options, the docs' order, averaged over 3 option rotations | **28 of 28** | **0.113** |
| the reconstructed layout (question JSON, answered with the key) | 16 to 17 of 28 | 0.30 |

What that refines:

- **The format needs the training.** Read with Jev's reconstructed layout,
  the local subject puts only 0.62 of its probability on the offered
  answers (0.98 with lettered options) and what remains is overconfident.
  The layout of findings 1 to 3 is Jev's input format, but it works for
  Jev because RLCD trained Jev on it; an untrained model answers far more
  like Jev when the question is a plain lettered choice.
- **Position bias is most of the remaining disagreement.** Keeping the
  docs' option order and averaging three rotations of it takes the local
  subject from 26 to all 28 of Jev's decisions.
- **Jev reads sharper.** The temperature that brings the local
  probabilities closest to Jev's is about 0.3 for Score and 0.7 for Choice
  and Noul (lower is sharper): Jev commits harder than the local subject,
  most of all on Scores. Eleven or fewer questions per kind: indicative.
- **Finding 6 measured.** Jev's Noul reads 0.22 where its yes/no Choice
  reads 0.01 on the same question, and a Noul and its negation sum to 1.19.
  The local subject, whose Noul is a two-option Choice, reads 0.03 and
  0.03, and 0.63 and 0.39 (1.02): a Noul answered as a Choice cannot
  disagree with its Choice twin. Jev's can, so its Noul is a different
  readout.
