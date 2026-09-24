//! Jev's server, reconstructed from TypeSafe's published documentation
//! alone: no call to Jev, no model. Each constant and step below is an
//! inference; `docs/reverse-engineering.md` gives the evidence and the
//! confidence for each, and `evidence/published_pairs.json` is the data
//! (the 13 request/response pairs TypeSafe publishes with token counts).
//!
//! What a request becomes, as inferred:
//!
//! 1. The document: a string state verbatim, a structured state as JSON
//!    indented by two spaces (what TypeSafe's playground sends,
//!    `JSON.stringify(state, null, 2)`).
//! 2. One branch per question: the question object as compact JSON
//!    (`type`, `instructions`, `criteria`; never the id, which "is not sent
//!    to the model").
//! 3. A fixed preamble of about 263 tokens and the document, prefilled
//!    once; every branch continues from it (the 32k limit is the preamble,
//!    the document and the longest branch; the 64k limit is all of them),
//!    and every branch is read in the same pass.
//! 4. Each branch yields a distribution over its outcomes (Choice options,
//!    Score levels, or the Noul's true), read from the model's output
//!    distribution: the published probabilities are not counts of a small
//!    number of samples.
//! 5. The answer: TypeSafe's confidence formulas, the Score's expected
//!    level, every number rounded to two decimals as Python's `round` does.
//!
//! See reconstruction.md.

use serde_json::{json, Map, Value};

use crate::confidence;
use crate::protocol::{parse_questions, Question, Request};

/// The fixed preamble, in tokens: the intercept of the fit of published
/// `input_tokens` against the text each request carries (docs).
pub const PREAMBLE_TOKENS: usize = 263;
/// Per-question wrapper tokens around a branch's JSON, by type (fit).
pub const WRAPPER_NOUL: usize = 6;
pub const WRAPPER_CHOICE: usize = 2;
pub const WRAPPER_SCORE: usize = 2;
/// "32k tokens for state plus the longest question".
pub const BRANCH_LIMIT: usize = 32_000;
/// "64k tokens per request".
pub const REQUEST_LIMIT: usize = 64_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Noul,
    Choice,
    Score,
}

/// One question's branch.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    /// The caller's id: kept for the answer, never shown to the model.
    pub id: String,
    pub kind: Kind,
    /// What the model reads for this question.
    pub prompt: String,
    /// What the readout is over: option keys, level numbers, or `true`.
    pub outcomes: Vec<String>,
    /// Score levels, for the answer's legend.
    pub legend: Vec<Value>,
}

/// A request as the server would run it.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub document: String,
    pub branches: Vec<Branch>,
}

/// The document a state becomes.
pub fn document(state: &Value) -> String {
    match state {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// Compile a request into its plan.
pub fn compile(req: &Request) -> Result<Plan, String> {
    let questions = parse_questions(&req.questions)?;
    let mut branches = Vec::with_capacity(questions.len());
    for (id, q) in questions {
        let raw = &req.questions[&id];
        let prompt = serde_json::to_string(raw).map_err(|e| e.to_string())?;
        let (kind, outcomes, legend) = match &q {
            Question::Noul { .. } => (Kind::Noul, vec!["true".to_string()], Vec::new()),
            Question::Choice { criteria, .. } => {
                (Kind::Choice, criteria.keys().cloned().collect(), Vec::new())
            }
            Question::Score { criteria, .. } => (
                Kind::Score,
                (0..criteria.len()).map(|i| i.to_string()).collect(),
                criteria.clone(),
            ),
        };
        branches.push(Branch {
            id,
            kind,
            prompt,
            outcomes,
            legend,
        });
    }
    Ok(Plan {
        document: document(&req.state),
        branches,
    })
}

/// Token accounting for a plan under a tokenizer (`count` gives a text's
/// token count): the billed input and the two limits.
#[derive(Debug, Clone, PartialEq)]
pub struct Budget {
    /// The inferred `usage.input_tokens`.
    pub input_tokens: usize,
    /// Preamble, document and the longest branch: at most 32k.
    pub longest_branch: usize,
}

pub fn budget(plan: &Plan, count: impl Fn(&str) -> usize) -> Result<Budget, String> {
    let shared = PREAMBLE_TOKENS + count(&plan.document);
    let mut total = shared;
    let mut longest = 0;
    for b in &plan.branches {
        let wrapper = match b.kind {
            Kind::Noul => WRAPPER_NOUL,
            Kind::Choice => WRAPPER_CHOICE,
            Kind::Score => WRAPPER_SCORE,
        };
        let n = count(&b.prompt) + wrapper;
        total += n;
        longest = longest.max(n);
    }
    if shared + longest > BRANCH_LIMIT {
        return Err(format!(
            "the state and the longest question are {} tokens; the limit is {BRANCH_LIMIT}",
            shared + longest
        ));
    }
    if total > REQUEST_LIMIT {
        return Err(format!(
            "the request is {total} tokens; the limit is {REQUEST_LIMIT}"
        ));
    }
    Ok(Budget {
        input_tokens: total,
        longest_branch: shared + longest,
    })
}

/// Two decimals the way Python's `round(x, 2)` does it: the exact binary
/// value rounded, ties to even (0.125 gives 0.12, 0.875 gives 0.88). Rust's
/// formatting rounds the same way.
pub fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// The answer a branch's distribution becomes. For a Noul, `probs` is one
/// value, P(true); otherwise one probability per outcome, in order.
pub fn assemble(branch: &Branch, probs: &[f64]) -> Value {
    match branch.kind {
        Kind::Noul => json!({"type": "noul", "noul": round2(probs[0])}),
        Kind::Choice => {
            let best = confidence::argmax(probs);
            let map: Map<String, Value> = branch
                .outcomes
                .iter()
                .zip(probs)
                .map(|(k, p)| (k.clone(), json!(round2(*p))))
                .collect();
            json!({
                "type": "choice",
                "choice": branch.outcomes[best],
                "confidence": round2(confidence::choice(probs)),
                "probabilities": map,
            })
        }
        Kind::Score => {
            let expected: f64 = probs.iter().enumerate().map(|(i, p)| i as f64 * p).sum();
            let legend: Map<String, Value> = branch
                .legend
                .iter()
                .enumerate()
                .map(|(i, l)| (i.to_string(), l.clone()))
                .collect();
            let map: Map<String, Value> = probs
                .iter()
                .enumerate()
                .map(|(i, p)| (i.to_string(), json!(round2(*p))))
                .collect();
            json!({
                "type": "score",
                "score": round2(expected),
                "confidence": round2(confidence::score(probs)),
                "legend": legend,
                "probabilities": map,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs() -> Vec<Value> {
        let text = include_str!("../evidence/published_pairs.json");
        serde_json::from_str::<Vec<Value>>(text).unwrap()
    }

    #[test]
    fn rounds_like_python() {
        assert_eq!(round2(0.125), 0.12);
        assert_eq!(round2(0.875), 0.88);
        assert_eq!(round2(0.925), 0.93);
        assert_eq!(round2(0.355), 0.35);
    }

    /// Every published Choice and Score answer: assembling from its own
    /// (rounded) probabilities reproduces its choice, confidence and score
    /// to within the rounding of the inputs.
    #[test]
    fn reproduces_every_published_answer() {
        let mut checked = 0;
        for p in pairs() {
            let req: Request = serde_json::from_value(p["request"].clone()).unwrap();
            let plan = compile(&req).unwrap();
            for b in &plan.branches {
                let a = &p["response"]["answers"][&b.id];
                if b.kind == Kind::Noul {
                    continue;
                }
                let probs: Vec<f64> = b
                    .outcomes
                    .iter()
                    .map(|k| a["probabilities"][k].as_f64().unwrap())
                    .collect();
                let mine = assemble(b, &probs);
                let near = |x: &Value, y: &Value| {
                    (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() <= 0.011
                };
                assert!(
                    near(&mine["confidence"], &a["confidence"]),
                    "{}: {mine} vs {a}",
                    b.id
                );
                if b.kind == Kind::Choice {
                    assert_eq!(mine["choice"], a["choice"], "{}", b.id);
                } else {
                    assert!(near(&mine["score"], &a["score"]), "{}", b.id);
                    assert_eq!(mine["legend"], a["legend"], "{}", b.id);
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 16);
    }

    #[test]
    fn ids_never_reach_the_model() {
        for p in pairs() {
            let req: Request = serde_json::from_value(p["request"].clone()).unwrap();
            for b in compile(&req).unwrap().branches {
                let v: Value = serde_json::from_str(&b.prompt).unwrap();
                let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
                assert!(
                    keys.iter()
                        .all(|k| ["type", "instructions", "criteria"].contains(&k.as_str())),
                    "{}: {keys:?}",
                    b.id
                );
            }
        }
    }

    #[test]
    fn a_long_branch_is_refused() {
        let req: Request = serde_json::from_value(json!({
            "state": "x",
            "questions": {"q": {"type": "noul", "instructions": "y"}}
        }))
        .unwrap();
        let plan = compile(&req).unwrap();
        assert!(budget(&plan, |s| s.len()).is_ok());
        assert!(budget(&plan, |_| 40_000).is_err());
    }
}
