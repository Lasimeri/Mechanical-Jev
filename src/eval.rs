//! Labelled case files scored by Jev, and what the scores say: accuracy,
//! Brier, calibration error, coverage at a 5 percent error budget, latency.
//! A case is one request plus `gold` (question id to the expected option
//! key, level, or `"yes"` / `"no"`). Every answer is kept as a `Row` (the
//! probability of each option in question order), which `--rows` writes and
//! `corroborate` reads back. See eval.md.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::client::{Client, JevError};
use crate::confidence::{self, argmax};
use crate::protocol::{parse_questions, Question, Request};

#[derive(Debug, Clone, Deserialize)]
pub struct Case {
    #[serde(default)]
    pub model: Option<String>,
    pub state: Value,
    pub questions: Map<String, Value>,
    pub gold: Map<String, Value>,
}

/// JSONL, `#` lines are comments.
pub fn load_cases(path: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(i, l)| {
            serde_json::from_str(l).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))
        })
        .collect()
}

/// One answered question with its gold option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub case_index: usize,
    pub id: String,
    pub kind: String,
    /// Option keys in question order (Noul: yes, no; Score: level numbers).
    pub keys: Vec<String>,
    /// Jev's probability of each key.
    pub probs: Vec<f64>,
    pub gold: usize,
    /// The request's time shared out among its questions.
    pub latency_ms: f64,
}

/// The option a gold label names: a key (a Score's level number is its
/// key), `true`/`false` or `"true"`/`"false"` for a Noul, else a number
/// read as an option index. `None` when it names no option, so a 1-based
/// level or a stray key is an error, never an index past the end.
fn gold_index(g: &Value, keys: &[String]) -> Option<usize> {
    let noul = keys.len() == 2 && keys[0] == "yes" && keys[1] == "no";
    let key = |s: &str| keys.iter().position(|k| k == s);
    let idx = match g {
        Value::Number(n) => key(&n.to_string()).or_else(|| n.as_u64().map(|x| x as usize)),
        Value::String(s) => key(s)
            .or(match s.as_str() {
                "true" if noul => Some(0),
                "false" if noul => Some(1),
                _ => None,
            })
            .or_else(|| s.parse().ok()),
        Value::Bool(b) if noul => Some(if *b { 0 } else { 1 }),
        _ => None,
    }?;
    (idx < keys.len()).then_some(idx)
}

/// Ask Jev every case. A case whose call fails for a reason other than the
/// request itself is counted as failed and skipped; an invalid case aborts.
pub fn run(client: &Client, cases: &[Case]) -> Result<(Vec<Row>, usize), JevError> {
    let mut rows = Vec::new();
    let mut failed = 0usize;
    for (ci, c) in cases.iter().enumerate() {
        let req = Request {
            model: c.model.clone(),
            state: c.state.clone(),
            questions: c.questions.clone(),
        };
        let (resp, took) = match client.system_one(&req) {
            Ok(r) => r,
            Err(
                e @ (JevError::Invalid(_) | JevError::Unprocessable(_) | JevError::Unauthorized),
            ) => return Err(e),
            Err(e) => {
                eprintln!("case {ci}: {e} (counted as failed)");
                failed += 1;
                continue;
            }
        };
        let questions = parse_questions(&c.questions).map_err(JevError::Invalid)?;
        let each = took.as_secs_f64() * 1e3 / questions.len().max(1) as f64;
        let mut case_rows = Vec::new();
        let mut malformed = None;
        for (id, q) in &questions {
            let Some((kind, keys, probs)) = read_answer(q, resp.answers.get(id)) else {
                malformed = Some(format!("no well-formed answer to `{id}`"));
                break;
            };
            let Some(g) = c.gold.get(id) else { continue };
            let gold = gold_index(g, &keys).ok_or_else(|| {
                JevError::Invalid(format!(
                    "case {ci} question `{id}`: gold {g} is not an option"
                ))
            })?;
            case_rows.push(Row {
                case_index: ci,
                id: id.clone(),
                kind: kind.into(),
                keys,
                probs,
                gold,
                latency_ms: each,
            });
        }
        match malformed {
            Some(why) => {
                eprintln!("case {ci}: {why} (counted as failed)");
                failed += 1;
            }
            None => rows.extend(case_rows),
        }
    }
    Ok((rows, failed))
}

/// One answer as `(kind, keys, probabilities)` in the question's order, or
/// `None` when it is missing, of another type, or lacks what its type
/// carries (a Noul's `noul`, every option's probability): a server's
/// malformed answer fails its case rather than being scored as a guess.
fn read_answer(q: &Question, a: Option<&Value>) -> Option<(&'static str, Vec<String>, Vec<f64>)> {
    let a = a?;
    let kind = match q {
        Question::Noul { .. } => "noul",
        Question::Choice { .. } => "choice",
        Question::Score { .. } => "score",
    };
    if a.get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| t != kind)
    {
        return None;
    }
    let probs_of = |keys: &[String]| -> Option<Vec<f64>> {
        let m = a.get("probabilities")?;
        keys.iter()
            .map(|k| m.get(k).and_then(Value::as_f64))
            .collect()
    };
    match q {
        Question::Noul { .. } => {
            let p = a.get("noul").and_then(Value::as_f64)?;
            Some((kind, vec!["yes".into(), "no".into()], vec![p, 1.0 - p]))
        }
        Question::Choice { criteria, .. } => {
            let keys: Vec<String> = criteria.keys().cloned().collect();
            let probs = probs_of(&keys)?;
            Some((kind, keys, probs))
        }
        Question::Score { criteria, .. } => {
            let keys: Vec<String> = (0..criteria.len()).map(|i| i.to_string()).collect();
            let probs = probs_of(&keys)?;
            Some((kind, keys, probs))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub questions: usize,
    pub failed_cases: usize,
    pub accuracy: f64,
    pub brier: f64,
    /// Top-label expected calibration error, 10 equal-width bins.
    pub ece: f64,
    pub mean_confidence: f64,
    /// The largest share of questions answerable, most confident first,
    /// with the error kept at or under 5 percent.
    pub coverage_at_5pct_error: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub accuracy_by_kind: BTreeMap<String, f64>,
}

pub fn metrics(rows: &[Row], failed_cases: usize) -> Metrics {
    let n = rows.len().max(1) as f64;
    let mut correct = 0usize;
    let mut brier = 0.0;
    let mut conf_sum = 0.0;
    let mut bins = vec![(0usize, 0usize, 0.0f64); 10];
    let mut gated: Vec<(f64, bool)> = Vec::new();
    let mut by_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let total: f64 = r.probs.iter().sum();
        let p: Vec<f64> = if total > 0.0 {
            r.probs.iter().map(|x| x / total).collect()
        } else {
            vec![1.0 / r.probs.len() as f64; r.probs.len()]
        };
        let pred = argmax(&p);
        let ok = pred == r.gold;
        correct += usize::from(ok);
        brier += p
            .iter()
            .enumerate()
            .map(|(i, &pi)| (pi - if i == r.gold { 1.0 } else { 0.0 }).powi(2))
            .sum::<f64>();
        let c = confidence::of(&r.kind, &p);
        conf_sum += c;
        let b = ((p[pred] * 10.0).floor() as usize).min(9);
        bins[b].0 += 1;
        bins[b].1 += usize::from(ok);
        bins[b].2 += p[pred];
        gated.push((c, ok));
        let e = by_kind.entry(r.kind.clone()).or_default();
        e.0 += 1;
        e.1 += usize::from(ok);
    }
    let ece = bins
        .iter()
        .filter(|(c, _, _)| *c > 0)
        .map(|(c, k, s)| (*c as f64 / n) * ((*k as f64 / *c as f64) - (s / *c as f64)).abs())
        .sum();
    // Coverage is what a confidence threshold accepts, and a threshold
    // cannot split rows of equal confidence: cut only where it changes.
    gated.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (mut best, mut wrong) = (0usize, 0usize);
    for (i, (c, ok)) in gated.iter().enumerate() {
        wrong += usize::from(!ok);
        let boundary = gated.get(i + 1).is_none_or(|next| next.0 != *c);
        if boundary && wrong as f64 / (i + 1) as f64 <= 0.05 {
            best = i + 1;
        }
    }
    let mut lat: Vec<f64> = rows.iter().map(|r| r.latency_ms).collect();
    lat.sort_by(f64::total_cmp);
    let pct = |q: f64| {
        if lat.is_empty() {
            0.0
        } else {
            lat[((lat.len() - 1) as f64 * q).round() as usize]
        }
    };
    Metrics {
        questions: rows.len(),
        failed_cases,
        accuracy: correct as f64 / n,
        brier: brier / n,
        ece,
        mean_confidence: conf_sum / n,
        coverage_at_5pct_error: best as f64 / n,
        latency_p50_ms: pct(0.5),
        latency_p95_ms: pct(0.95),
        accuracy_by_kind: by_kind
            .into_iter()
            .map(|(k, (t, c))| (k, c as f64 / t.max(1) as f64))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, probs: &[f64], gold: usize) -> Row {
        Row {
            case_index: 0,
            id: "q".into(),
            kind: kind.into(),
            keys: (0..probs.len()).map(|i| i.to_string()).collect(),
            probs: probs.to_vec(),
            gold,
            latency_ms: 100.0,
        }
    }

    #[test]
    fn metrics_count_right_and_wrong() {
        let rows = vec![
            row("noul", &[0.9, 0.1], 0),
            row("choice", &[0.2, 0.7, 0.1], 1),
            row("score", &[0.6, 0.3, 0.1], 2),
        ];
        let m = metrics(&rows, 0);
        assert_eq!(m.questions, 3);
        assert!((m.accuracy - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(m.accuracy_by_kind["score"], 0.0);
        assert!(m.brier > 0.0 && m.ece >= 0.0);
    }

    #[test]
    fn a_gold_label_names_an_option_or_nothing() {
        use serde_json::json;
        let k = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(gold_index(&json!(true), &k(&["yes", "no"])), Some(0));
        assert_eq!(gold_index(&json!("false"), &k(&["yes", "no"])), Some(1));
        assert_eq!(gold_index(&json!(3), &k(&["0", "1", "2"])), None);
        assert_eq!(gold_index(&json!("5"), &k(&["0", "1", "2"])), None);
        assert_eq!(gold_index(&json!(32), &k(&["16", "32", "64"])), Some(1));
    }

    #[test]
    fn coverage_does_not_split_tied_confidences() {
        let rows: Vec<Row> = (0..20)
            .map(|i| row("noul", &[0.9, 0.1], usize::from(i >= 10)))
            .collect();
        assert_eq!(metrics(&rows, 0).coverage_at_5pct_error, 0.0);
    }

    #[test]
    fn a_malformed_answer_is_not_scored_as_a_guess() {
        use serde_json::json;
        let choice: Question = serde_json::from_value(
            json!({"type": "choice", "instructions": "?", "criteria": {"a": null, "b": null}}),
        )
        .unwrap();
        let noul: Question =
            serde_json::from_value(json!({"type": "noul", "instructions": "?"})).unwrap();
        assert!(read_answer(&choice, None).is_none());
        assert!(read_answer(&choice, Some(&json!({"type": "choice", "choice": "a"}))).is_none());
        assert!(read_answer(&choice, Some(&json!({"type": "noul", "noul": 0.5}))).is_none());
        assert!(read_answer(&noul, Some(&json!({"type": "noul"}))).is_none());
        let ok = read_answer(
            &choice,
            Some(&json!({"type": "choice", "probabilities": {"a": 0.7, "b": 0.3}})),
        )
        .unwrap();
        assert_eq!(ok.2, vec![0.7, 0.3]);
    }
}
