//! Jev's published answers as something to measure against. TypeSafe's docs
//! print complete request and response pairs
//! (`evidence/published_pairs.json`) and a few structural oddities
//! (`evidence/invariants.json`). `cases` turns them into a labelled case
//! file whose gold is Jev's own answer, and `jev_rows` into the rows `eval`
//! would have recorded for Jev, so `mjev eval` against any server and
//! `mjev corroborate` say how closely that server answers like Jev.
//! See evidence.md.

use serde_json::{json, Map, Value};

use crate::confidence::argmax;
use crate::eval::Row;
use crate::protocol::{parse_questions, Question};

const PAIRS: &str = include_str!("../evidence/published_pairs.json");
const INVARIANTS: &str = include_str!("../evidence/invariants.json");

/// Every published pair, the invariant examples after them.
pub fn all() -> Vec<Value> {
    let mut v: Vec<Value> = serde_json::from_str(PAIRS).unwrap_or_default();
    v.extend(serde_json::from_str::<Vec<Value>>(INVARIANTS).unwrap_or_default());
    v
}

/// Jev's probability of each outcome of one question, in question order,
/// with the outcome keys (Noul: yes, no).
fn jev_probs(q: &Question, a: &Value) -> (String, Vec<String>, Vec<f64>) {
    let p = |k: &str| a["probabilities"][k].as_f64().unwrap_or(0.0);
    match q {
        Question::Noul { .. } => {
            let y = a["noul"].as_f64().unwrap_or(0.5);
            (
                "noul".into(),
                vec!["yes".into(), "no".into()],
                vec![y, 1.0 - y],
            )
        }
        Question::Choice { criteria, .. } => {
            let keys: Vec<String> = criteria.keys().cloned().collect();
            let probs = keys.iter().map(|k| p(k)).collect();
            ("choice".into(), keys, probs)
        }
        Question::Score { criteria, .. } => {
            let keys: Vec<String> = (0..criteria.len()).map(|i| i.to_string()).collect();
            let probs = keys.iter().map(|k| p(k)).collect();
            ("score".into(), keys, probs)
        }
    }
}

/// The case file: each request, with Jev's answer as its gold.
pub fn cases() -> Vec<Value> {
    all()
        .iter()
        .map(|e| {
            let req = &e["request"];
            let qs = req["questions"].as_object().cloned().unwrap_or_default();
            let parsed = parse_questions(&qs).unwrap_or_default();
            let mut gold = Map::new();
            for (id, q) in &parsed {
                let (_, keys, probs) = jev_probs(q, &e["response"]["answers"][id]);
                gold.insert(id.clone(), json!(keys[argmax(&probs)]));
            }
            json!({"state": req["state"], "questions": qs, "gold": gold})
        })
        .collect()
}

/// Jev's published answers as `eval` rows.
pub fn jev_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for (ci, e) in all().iter().enumerate() {
        let qs = e["request"]["questions"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        for (id, q) in parse_questions(&qs).unwrap_or_default() {
            let (kind, keys, probs) = jev_probs(&q, &e["response"]["answers"][&id]);
            let gold = argmax(&probs);
            rows.push(Row {
                case_index: ci,
                id,
                kind,
                keys,
                probs,
                gold,
                latency_ms: 0.0,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_published_question_becomes_a_row() {
        let rows = super::jev_rows();
        assert_eq!(rows.len(), 28); // 24 questions in 13 pairs, 4 invariant questions
        assert_eq!(super::cases().len(), 15);
        assert!(rows.iter().all(|r| r.gold < r.probs.len()));
    }
}
