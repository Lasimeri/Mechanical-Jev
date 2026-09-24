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
        let pb = aligned(ra, rb);
        let max_prob_diff = ra
            .probs
            .iter()
            .zip(&pb)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        let (xa, xb) = (argmax(&ra.probs), argmax(&pb));
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

/// `p` at temperature `t`: `p^(1/t)` renormalised (above one softer,
/// below one sharper).
pub fn temper(p: &[f64], t: f64) -> Vec<f64> {
    let q: Vec<f64> = p.iter().map(|x| x.max(1e-12).powf(1.0 / t)).collect();
    let s: f64 = q.iter().sum();
    q.iter().map(|x| x / s).collect()
}

/// For each question kind, the temperature that brings B's probabilities
/// closest to A's (the mean of the largest per-option difference): how much
/// softer or sharper one system reads than the other.
#[derive(Debug, Clone, Serialize)]
pub struct Tempered {
    pub kind: String,
    pub questions: usize,
    pub temperature: f64,
    pub mean_prob_diff_before: f64,
    pub mean_prob_diff_after: f64,
}

pub fn fit_temperature(a: &[Row], b: &[Row]) -> Vec<Tempered> {
    let key = |r: &Row| (r.case_index, r.id.clone());
    let bm: BTreeMap<_, &Row> = b.iter().map(|r| (key(r), r)).collect();
    let mut by_kind: BTreeMap<String, Vec<(&Row, &Row)>> = BTreeMap::new();
    for ra in a {
        if let Some(rb) = bm.get(&key(ra)) {
            by_kind.entry(ra.kind.clone()).or_default().push((ra, rb));
        }
    }
    let diff = |pairs: &[(&Row, &Row)], t: f64| -> f64 {
        pairs
            .iter()
            .map(|(ra, rb)| {
                temper(&aligned(ra, rb), t)
                    .iter()
                    .zip(&ra.probs)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0, f64::max)
            })
            .sum::<f64>()
            / pairs.len().max(1) as f64
    };
    by_kind
        .into_iter()
        .map(|(kind, pairs)| {
            let grid = (0..=60).map(|i| 0.2 * 1.08f64.powi(i));
            let (t, d) = grid
                .map(|t| (t, diff(&pairs, t)))
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .unwrap_or((1.0, 0.0));
            Tempered {
                kind,
                questions: pairs.len(),
                temperature: (t * 100.0).round() / 100.0,
                mean_prob_diff_before: diff(&pairs, 1.0),
                mean_prob_diff_after: d,
            }
        })
        .collect()
}

/// B's probabilities in A's option order (matched by key; a key B lacks
/// reads as 0), so two runs that listed options differently compare.
pub fn aligned(ra: &Row, rb: &Row) -> Vec<f64> {
    ra.keys
        .iter()
        .map(|k| {
            rb.keys
                .iter()
                .position(|x| x == k)
                .map(|i| rb.probs[i])
                .unwrap_or(0.0)
        })
        .collect()
}
