//! TypeSafe's confidence formulas, as its MIT-licensed `system-one-adapter`
//! computes them (`_utils/confidence_metrics.py`), with its test cases.
//! Jev returns `confidence` on every Choice and Score; these reproduce it
//! from `probabilities`, and let a caller compute it for any distribution.
//! See confidence.md.

fn normalized(probs: &[f64]) -> Vec<f64> {
    let total: f64 = probs.iter().sum();
    if total <= 0.0 {
        return vec![1.0 / probs.len() as f64; probs.len()];
    }
    probs.iter().map(|p| p / total).collect()
}

/// The index of the largest value, the first on a tie.
pub fn argmax(probs: &[f64]) -> usize {
    let mut best = 0;
    for (i, &p) in probs.iter().enumerate() {
        if p > probs[best] {
            best = i;
        }
    }
    best
}

/// Choice: the peak probability scaled from uniform (0) to certainty (1),
/// `(p_max - 1/n) / (1 - 1/n)`.
pub fn choice(probs: &[f64]) -> f64 {
    if probs.len() < 2 {
        return 1.0;
    }
    let p = normalized(probs);
    let u = 1.0 / p.len() as f64;
    let pmax = p.iter().copied().fold(0.0, f64::max);
    ((pmax - u) / (1.0 - u)).clamp(0.0, 1.0)
}

/// Score: concentration around the modal level, one minus the mean
/// distance from the mode over a uniform distribution's mean absolute
/// deviation. A near miss costs less than a far one.
pub fn score(probs: &[f64]) -> f64 {
    if probs.len() < 2 {
        return 1.0;
    }
    let p = normalized(probs);
    let mode = argmax(&p) as f64;
    let spread: f64 = p
        .iter()
        .enumerate()
        .map(|(i, q)| q * (i as f64 - mode).abs())
        .sum();
    let n = p.len() as f64;
    let center = (n - 1.0) / 2.0;
    let uniform_mad: f64 = (0..p.len()).map(|i| (i as f64 - center).abs()).sum::<f64>() / n;
    (1.0 - spread / uniform_mad).max(0.0)
}

/// By question kind (`"score"` or anything else).
pub fn of(kind: &str, probs: &[f64]) -> f64 {
    if kind == "score" {
        score(probs)
    } else {
        choice(probs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_typesafe() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(score(&[0.2; 5]), 0.0));
        assert!(close(score(&[0.04; 5]), 0.0));
        assert!(close(score(&[0.01, 0.02, 0.07, 0.3, 0.6]), 0.55));
        assert!(close(choice(&[0.5, 0.5]), 0.0));
        assert!(close(choice(&[0.2, 0.2]), 0.0));
        assert!(close(choice(&[0.82, 0.18]), 0.64));
        assert!(close(score(&[1.0]), 1.0));
        assert!(close(choice(&[1.0]), 1.0));
        assert!(close(choice(&[0.6, 0.3, 0.1]), 0.4));
    }
}
