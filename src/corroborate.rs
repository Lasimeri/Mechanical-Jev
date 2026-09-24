//! Two recorded runs (`mjev eval --rows`) compared question by question:
//! Jev today against Jev last week (does a new model version move the
//! answers?), one model version against another, or Jev against any other
//! system that writes the same rows. See corroborate.md.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::confidence::argmax;
use crate::eval::Row;

#[derive(Debug, Clone, Serialize)]
pub struct Pair {
    pub case_index: usize,
    pub id: String,
    pub max_prob_diff: f64,
    pub same_answer: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub matched: usize,
    pub only_in_a: usize,
    pub only_in_b: usize,
    pub same_answer: usize,
    pub max_prob_diff: f64,
    pub mean_prob_diff: f64,
    pub accuracy_a: f64,
    pub accuracy_b: f64,
    pub pairs: Vec<Pair>,
}

pub fn compare(a: &[Row], b: &[Row]) -> Report {
    let key = |r: &Row| (r.case_index, r.id.clone());
    let bm: BTreeMap<_, &Row> = b.iter().map(|r| (key(r), r)).collect();
    let mut pairs = Vec::new();
    let (mut ok_a, mut ok_b) = (0usize, 0usize);
    for ra in a {
        let Some(rb) = bm.get(&key(ra)) else { continue };
        let max_prob_diff = ra
            .probs
            .iter()
            .zip(&rb.probs)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        let (xa, xb) = (argmax(&ra.probs), argmax(&rb.probs));
        ok_a += usize::from(xa == ra.gold);
        ok_b += usize::from(xb == rb.gold);
        pairs.push(Pair {
            case_index: ra.case_index,
            id: ra.id.clone(),
            max_prob_diff,
            same_answer: xa == xb,
        });
    }
    let n = pairs.len().max(1) as f64;
    Report {
        matched: pairs.len(),
        only_in_a: a.len() - pairs.len(),
        only_in_b: b.len() - pairs.len(),
        same_answer: pairs.iter().filter(|p| p.same_answer).count(),
        max_prob_diff: pairs.iter().map(|p| p.max_prob_diff).fold(0.0, f64::max),
        mean_prob_diff: pairs.iter().map(|p| p.max_prob_diff).sum::<f64>() / n,
        accuracy_a: ok_a as f64 / n,
        accuracy_b: ok_b as f64 / n,
        pairs,
    }
}
