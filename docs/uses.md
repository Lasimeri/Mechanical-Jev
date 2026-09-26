# What people use Jev for, and what the family does about it

Found 2026-09-26 with Exa: TypeSafe's own documentation (docs.typesafe.ai,
the spec) and launch post, and write-ups by people and teams using Jev.
The write-ups and the unofficial sites (jevtypesafeai.com, learnjev.com,
systemonemodels.org and the like) are evidence of use only: no behaviour
here is taken from them. Each feature names the docs.typesafe.ai page it
follows in its own `.md`.

## What they do with it

| use | who says so | the pattern underneath |
| --- | --- | --- |
| act on sure answers, send the unsure to a person | docs: `confidence.md`, `patterns/confidence-routing.md`; every write-up | confidence-gated routing: three bands, thresholds by risk |
| support triage, email triage, moderation | docs: `patterns/intent-routing.md`, `patterns/fan-out.md`; LangChain, DigitalOcean, Akka | many questions about one state in one call; route in code |
| model routing (cheap model or expensive one) | LangChain (router middleware), Seif Ibrahim (Spring AI), Akka gateway | a Choice over difficulty, gated on confidence, a fallback floor |
| guarding an agent's tool calls | LangChain (AutoModeMiddleware), Nym (seven reviewers replaced), Akka | a risk question before a tool runs: allow, confirm or block |
| judging another model's output | docs: cookbooks for LLM guardrails and citations; Seif Ibrahim's answer reviewer; LangChain's trace scoring | a Noul for grounding and a Score for completeness, thresholds in code |
| ranking and filtering search results, RAG passages | docs: cookbooks for re-ranking, line-by-line search, classifying RAG passages; elvex | one question per query and candidate, sorted in code |
| labelling data in bulk | elvex (2,000 expense reports in 21 s); Aman Kumar (16,000 calls) | the same questions over many states |
| a cheap filter in front of a costlier classifier | Aman Kumar ("classifier or filter?") | trust the confident ends, set the drop line from known positives, fall back in the middle |
| composite scores (lead fit, résumé ranking) | docs: `patterns/composite-scoring.md` | atomic Scores weighed in code, the dimensions kept |
| keeping a record of each decision | ArchCrux, Bloss0m | the state, question, answer and policy of every decision, replayable |
| real-time loops (games, browsers at 10 requests a second) | TypeSafe's DOOM demo; Browserbase; Nym | needs 70 to 500 ms per decision |

## What the family has, builds, or skips

| pattern | where | status |
| --- | --- | --- |
| many questions in one request (speculative fan-out) | Intel Phi Jev's `xks`: the state read once, every question forked from it | covered |
| Choices past 26 options, up to 255 | `xks`'s trie of forks (its subproject 05) | covered |
| an MCP server | `xks mcp` | covered |
| calibration from labelled cases | `mjev eval`, `xks eval`, `xks condition`, `xks replay` | covered |
| confidence-gated routing, composites, speculative answers ignored | [`src/policy.rs`](../src/policy.md), `mjev gate` (exit 0 act, 10 review, 11 escalate) | built |
| the same questions over many states | [`src/label.rs`](../src/label.md), `mjev label` | built |
| rank candidates against a query | [`src/rank.rs`](../src/rank.md), `mjev rank` | built |
| thresholds from your own data, not copied | [`src/fit.rs`](../src/fit.md), `mjev fit` | built |
| a record of every decision | `xks serve` with a decision log | planned in this pass |
| guarding Claude Code's tool calls | a PreToolUse hook | planned in this pass, not installed |
| real-time loops | | skipped: this hardware answers in seconds (the 2B 0.65 to 2.1 s on x86 for `mjev gate`, the 35B 7 to 45 s in the subprojects), not 70 to 500 ms |
| context compaction for coding agents | | skipped: DigitalOcean notes it discards the warm cache it would save |
| text extraction | | skipped: Jev does not generate (TypeSafe's jaggedness notes, "Generation"); code or an LLM extracts, a Choice picks |

## Sources

- TypeSafe: [docs.typesafe.ai](https://docs.typesafe.ai/llms.txt) (the
  confidence page, the four patterns, the cookbooks, the jev-1.13
  jaggedness notes) and
  [the launch post](https://typesafe.ai/blog/introducing-system-one-models-and-jev).
- Write-ups: LangChain
  ([building a harness with Jev](https://www.langchain.com/blog/building-a-harness-with-jev)),
  Nym ([rebuilding our agent](https://usenym.com/technical-blog/rebuilding-our-agent-with-jev)),
  Aman Kumar ([classifier or filter?](https://amankumar.ai/blogs/jev-measured)),
  elvex ([harness UX](https://www.elvex.com/blog/early-experimentation-using-jev-to-rethink-harness-ux)),
  Akka ([fast, cheap agent decisions](https://akka.io/blog/fast-cheap-agent-decisions)),
  ArchCrux ([production systems](https://www.archcrux.com/articles/designing-production-ai-systems-with-jev)),
  Seif Ibrahim ([Spring AI](https://iseif.dev/2026/09/24/jev-with-spring-ai-model-routing-and-answer-review/)),
  Bloss0m ([agent runtime](https://www.bloss0m.com/en/blog/108-jev-confidence-gated-agent-runtime/)),
  DigitalOcean ([what is Jev](https://www.digitalocean.com/resources/articles/what-is-jev)).
