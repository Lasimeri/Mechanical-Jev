//! Thresholds from recorded answers and what they should have been:
//! `mjev fit`. Where each question's answers can be trusted, and a policy
//! that acts only there. No model is asked. See fit.md.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{json, Value};

use crate::confidence;
use crate::eval::Row;
use crate::policy::{Policy, Rule, ACT_AT, NO_AT, YES_AT};

/// A band of answers: how many fell in it (and their share) and how many
/// of those were right.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Band {
    pub n: usize,
    pub share: f64,
    /// `None` when the band is empty.
    pub accuracy: Option<f64>,
}

fn band(hits: &[bool], total: usize) -> Band {
    let n = hits.len();
    Band {
        n,
        share: if total == 0 {
            0.0
        } else {
            n as f64 / total as f64
        },
        accuracy: (n > 0).then(|| hits.iter().filter(|h| **h).count() as f64 / n as f64),
    }
}

/// One question's record, and the rule it suggests.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fitted {
    pub id: String,
    pub kind: String,
    pub n: usize,
    /// The argmax right (a Noul cut at 0.5).
    pub accuracy: f64,
    /// Noul: at or under 0.1, between, at or over 0.9 (TypeSafe's example
    /// band); Choice and Score: confidence under 0.5, between, from 0.9.
    pub low: Band,
    pub middle: Band,
    pub high: Band,
    /// Noul: half the lowest P(yes) any real yes was given; under it no
    /// yes was lost on these rows, and `dropped` of all answers fall.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_line: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped: Option<f64>,
    /// The rule suggested, with what acting on it would have done.
    pub rule: Rule,
    pub note: String,
}

/// z for a one-sided 95 percent bound.
const Z: f64 = 1.645;

/// The lower end of what `right` of `n` says the true share right may be:
/// the Wilson score bound, one-sided at 95 percent. 24 right of 25 is
/// 0.96 seen but 0.84 at worst; 60 of 60 is 0.957 at worst.
pub fn lower_bound(right: usize, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let (p, n) = (right as f64 / n as f64, n as f64);
    let z2 = Z * Z;
    let centre = p + z2 / (2.0 * n);
    let spread = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    (centre - spread) / (1.0 + z2 / n)
}

/// The lowest threshold `t` on a 0.01 grid from `from` to 0.99 where the
/// answers `score >= t` are right `target` of the time even at the lower
/// bound of what they show (`lower_bound`), with its coverage (share of
/// all answers at or over it). A few lucky answers do not set a threshold.
fn lowest_meeting(
    scored: &[(f64, bool)],
    from: f64,
    target: f64,
    min_support: usize,
) -> Option<(f64, f64)> {
    let total = scored.len();
    (0..100)
        .map(|i| i as f64 / 100.0)
        .filter(|t| *t >= from - 1e-9)
        .find_map(|t| {
            let over: Vec<bool> = scored
                .iter()
                .filter(|(s, _)| *s >= t)
                .map(|x| x.1)
                .collect();
            let right = over.iter().filter(|h| **h).count();
            (over.len() >= min_support && lower_bound(right, over.len()) >= target)
                .then(|| (t, over.len() as f64 / total as f64))
        })
}

/// Fit every question in `rows`. `target`: the accuracy an answer acted
/// on must reach on these rows.
pub fn fit(rows: &[Row], target: f64) -> Vec<Fitted> {
    let mut by: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by.entry((r.id.clone(), r.kind.clone()))
            .or_default()
            .push(r);
    }
    by.into_iter()
        .map(|((id, kind), rs)| {
            if kind == "noul" {
                noul(id, &rs, target)
            } else {
                graded(id, kind, &rs, target)
            }
        })
        .collect()
}

/// Enough rows for a threshold to mean something at all.
const FEW: usize = 30;
/// Under this many answers a question keeps the default thresholds:
/// fitted to one or two answers, a threshold only restates them.
pub const MIN_FIT: usize = 10;

fn noul(id: String, rs: &[&Row], target: f64) -> Fitted {
    let total = rs.len();
    // (P(yes), was yes)
    let pts: Vec<(f64, bool)> = rs
        .iter()
        .map(|r| (r.probs.first().copied().unwrap_or(0.0), r.gold == 0))
        .collect();
    let right = |(p, yes): &(f64, bool)| (*p >= 0.5) == *yes;
    let accuracy = pts.iter().filter(|x| right(x)).count() as f64 / total.max(1) as f64;
    let low = band(
        &pts.iter()
            .filter(|(p, _)| *p <= NO_AT)
            .map(|(_, y)| !y)
            .collect::<Vec<_>>(),
        total,
    );
    let high = band(
        &pts.iter()
            .filter(|(p, _)| *p >= YES_AT)
            .map(|(_, y)| *y)
            .collect::<Vec<_>>(),
        total,
    );
    let middle = band(
        &pts.iter()
            .filter(|(p, _)| *p > NO_AT && *p < YES_AT)
            .map(right)
            .collect::<Vec<_>>(),
        total,
    );
    let min_yes = pts
        .iter()
        .filter(|(_, y)| *y)
        .map(|(p, _)| *p)
        .fold(f64::INFINITY, f64::min);
    let (drop_line, dropped) = if min_yes.is_finite() {
        let line = min_yes / 2.0;
        let under = pts.iter().filter(|(p, _)| *p < line).count();
        (Some(line), Some(under as f64 / total as f64))
    } else {
        (None, None)
    };
    // yes_at: the lowest P(yes) over which the answers are yes `target` of
    // the time; no_at: the highest under which they are no as often.
    let support = 3.min(total);
    let fits = total >= MIN_FIT;
    let yes_at = lowest_meeting(&pts, 0.5, target, support)
        .map(|(t, _)| t)
        .filter(|_| fits);
    let flipped: Vec<(f64, bool)> = pts.iter().map(|(p, y)| (1.0 - p, !y)).collect();
    let no_at = lowest_meeting(&flipped, 0.5, target, support)
        .map(|(t, _)| 1.0 - t)
        .filter(|_| fits);
    let mut notes = Vec::new();
    if !fits {
        notes.push(format!(
            "{total} answers, under the {MIN_FIT} a fit needs: the default thresholds"
        ));
    } else if yes_at.is_none() {
        notes.push(format!(
            "no P(yes) is surely (at its lower bound) {target} right on yes: yes_at left at {YES_AT}"
        ));
    }
    if fits && no_at.is_none() {
        notes.push(format!(
            "no P(yes) is surely (at its lower bound) {target} right on no: no_at left at {NO_AT}"
        ));
    }
    Fitted {
        id,
        kind: "noul".into(),
        n: total,
        accuracy,
        low,
        middle,
        high,
        drop_line,
        dropped,
        rule: Rule {
            yes_at: Some(round2(yes_at.unwrap_or(YES_AT))),
            no_at: Some(round2(no_at.unwrap_or(NO_AT))),
            ..Rule::default()
        },
        note: few(total, notes),
    }
}

fn graded(id: String, kind: String, rs: &[&Row], target: f64) -> Fitted {
    let total = rs.len();
    // (confidence, argmax right)
    let pts: Vec<(f64, bool)> = rs
        .iter()
        .map(|r| {
            (
                confidence::of(&kind, &r.probs),
                confidence::argmax(&r.probs) == r.gold,
            )
        })
        .collect();
    let accuracy = pts.iter().filter(|x| x.1).count() as f64 / total.max(1) as f64;
    let pick = |f: &dyn Fn(f64) -> bool| {
        pts.iter()
            .filter(|(c, _)| f(*c))
            .map(|x| x.1)
            .collect::<Vec<_>>()
    };
    let low = band(&pick(&|c| c < 0.5), total);
    let middle = band(&pick(&|c| (0.5..ACT_AT).contains(&c)), total);
    let high = band(&pick(&|c| c >= ACT_AT), total);
    let act = lowest_meeting(&pts, 0.0, target, 3.min(total));
    let mut notes = Vec::new();
    let act_at = match act.filter(|_| total >= MIN_FIT) {
        Some((t, cover)) => {
            notes.push(format!(
                "acting from confidence {t:.2} covers {:.0} percent of answers",
                cover * 100.0
            ));
            t
        }
        None if total < MIN_FIT => {
            notes.push(format!(
                "{total} answers, under the {MIN_FIT} a fit needs: the default thresholds"
            ));
            ACT_AT
        }
        None => {
            notes.push(format!(
                "no confidence is surely (at its lower bound) {target} right: act_at left at {ACT_AT}, check this question by hand"
            ));
            ACT_AT
        }
    };
    Fitted {
        id,
        kind,
        n: total,
        accuracy,
        low,
        middle,
        high,
        drop_line: None,
        dropped: None,
        rule: Rule {
            act_at: Some(round2(act_at)),
            // The floor stays under the bar to act.
            floor: Some(round2(act_at.min(0.5))),
            ..Rule::default()
        },
        note: few(total, notes),
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn few(n: usize, mut notes: Vec<String>) -> String {
    if n < FEW {
        notes.push(format!(
            "only {n} answers: a starting point, not a threshold to trust; set it on part of your data and check it on the rest"
        ));
    }
    notes.join("; ")
}

/// The suggested policy: each fitted question's rule.
pub fn policy(fitted: &[Fitted]) -> Policy {
    Policy {
        questions: fitted
            .iter()
            .map(|f| (f.id.clone(), f.rule.clone()))
            .collect(),
        ..Policy::default()
    }
}

/// The report a person reads.
pub fn text(fitted: &[Fitted], target: f64) -> String {
    let pct = |b: &Band| {
        format!(
            "{:>3.0}% of answers, {}",
            b.share * 100.0,
            b.accuracy
                .map_or("none".to_string(), |a| format!("{:.0}% right", a * 100.0))
        )
    };
    let mut out = format!(
        "target: acted-on answers right {:.0}% of the time\n",
        target * 100.0
    );
    for f in fitted {
        let (lo, hi) = if f.kind == "noul" {
            ("p(yes) <= 0.10", "p(yes) >= 0.90")
        } else {
            ("confidence < 0.50", "confidence >= 0.90")
        };
        out.push_str(&format!(
            "\n{} ({}, {} answers, {:.0}% right)\n  {lo:<18} {}\n  {:<18} {}\n  {hi:<18} {}\n",
            f.id,
            f.kind,
            f.n,
            f.accuracy * 100.0,
            pct(&f.low),
            "between",
            pct(&f.middle),
            pct(&f.high)
        ));
        if let (Some(l), Some(d)) = (f.drop_line, f.dropped) {
            out.push_str(&format!(
                "  drop line {l:.3}: no yes under it here; {:.0}% of answers fall under it\n",
                d * 100.0
            ));
        }
        let rule = serde_json::to_value(&f.rule).unwrap_or(Value::Null);
        let set: serde_json::Map<String, Value> = rule
            .as_object()
            .map(|o| {
                o.iter()
                    .filter(|(_, v)| !v.is_null() && *v != &json!(false) && *v != &json!({}))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        out.push_str(&format!("  suggests {}\n", Value::Object(set)));
        if !f.note.is_empty() {
            out.push_str(&format!("  {}\n", f.note));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noul_row(id: &str, p: f64, yes: bool) -> Row {
        Row {
            case_index: 0,
            id: id.into(),
            kind: "noul".into(),
            keys: vec!["yes".into(), "no".into()],
            probs: vec![p, 1.0 - p],
            gold: if yes { 0 } else { 1 },
            latency_ms: 0.0,
        }
    }

    fn choice_row(probs: &[f64], gold: usize) -> Row {
        Row {
            case_index: 0,
            id: "c".into(),
            kind: "choice".into(),
            keys: (0..probs.len()).map(|i| i.to_string()).collect(),
            probs: probs.to_vec(),
            gold,
            latency_ms: 0.0,
        }
    }

    #[test]
    fn a_noul_gets_its_drop_line_from_its_positives_and_its_bands() {
        // Obviously artificial readings, each 20 times: yes cases high, one
        // yes at 0.40; no cases low, one no at 0.60.
        let mut rows = vec![];
        for _ in 0..20 {
            for p in [0.99, 0.97, 0.95, 0.92, 0.40] {
                rows.push(noul_row("q", p, true));
            }
            for p in [0.02, 0.03, 0.05, 0.08, 0.30, 0.60] {
                rows.push(noul_row("q", p, false));
            }
        }
        let f = &fit(&rows, 0.95)[0];
        assert_eq!(f.n, 220);
        assert_eq!(f.drop_line, Some(0.20));
        // Under 0.20: 0.02, 0.03, 0.05, 0.08 (all no), 4 of 11.
        assert!((f.dropped.unwrap() - 4.0 / 11.0).abs() < 1e-9);
        assert_eq!(f.high.accuracy, Some(1.0));
        assert_eq!(f.low.n, 80);
        // yes_at: from 0.61 up only yeses remain (80 of 80, 0.967 at the
        // lower bound); no_at: under 0.40, only nos (100 of 100).
        assert_eq!(f.rule.yes_at, Some(0.61));
        assert_eq!(f.rule.no_at, Some(0.39));
        assert!(!f.note.contains("only"), "{}", f.note);
    }

    #[test]
    fn a_lucky_few_do_not_set_a_threshold() {
        // 24 right of 25 is 0.96 seen, 0.84 at the lower bound: not 0.95.
        assert!(lower_bound(24, 25) < 0.95);
        assert!((lower_bound(24, 25) - 0.839).abs() < 0.002);
        assert!(lower_bound(60, 60) >= 0.95);
        assert_eq!(lower_bound(0, 0), 0.0);
        let rows: Vec<Row> = (0..25)
            .map(|i| choice_row(&[0.8, 0.2], if i == 0 { 1 } else { 0 }))
            .collect();
        let f = &fit(&rows, 0.95)[0];
        assert_eq!(f.accuracy, 0.96);
        assert_eq!(f.rule.act_at, Some(ACT_AT));
    }

    #[test]
    fn a_choice_acts_from_the_lowest_confidence_that_is_right_enough() {
        // Each reading 20 times.
        let rows: Vec<Row> = [(0.95, 0), (0.90, 0), (0.85, 0), (0.60, 1), (0.55, 1)]
            .iter()
            .flat_map(|&(p, g)| (0..20).map(move |_| choice_row(&[p, 1.0 - p], g)))
            .collect();
        let f = &fit(&rows, 0.95)[0];
        assert_eq!(f.accuracy, 0.6);
        // Two-option confidence is 2p - 1: the right answers sit at 0.90,
        // 0.80 and 0.70, the wrong ones at 0.20 and 0.10 (as floats, 2 *
        // 0.60 - 1 is a hair under 0.20). The bar lands on the first grid
        // step past the highest wrong one, well under the lowest right one.
        let wrong = confidence::of("choice", &[0.60, 0.40]);
        let act = f.rule.act_at.unwrap();
        assert!(act > wrong && act <= wrong + 0.011, "{act} against {wrong}");
        assert!(f.note.contains("covers 60 percent"), "{}", f.note);
        // The suggested policy passes the policy's own checks.
        policy(std::slice::from_ref(f)).check().unwrap();
    }

    #[test]
    fn a_question_never_right_enough_keeps_the_default_and_says_so() {
        let rows: Vec<Row> = (0..10).map(|_| choice_row(&[0.9, 0.1], 1)).collect();
        let f = &fit(&rows, 0.95)[0];
        assert_eq!(f.rule.act_at, Some(ACT_AT));
        assert!(f.note.contains("check this question by hand"));
    }

    #[test]
    fn too_few_answers_keep_the_defaults_and_say_why() {
        // One answer, right and sure: a fit would put yes_at at 0.5.
        let f = &fit(&[noul_row("q", 0.97, true)], 0.95)[0];
        assert_eq!((f.rule.yes_at, f.rule.no_at), (Some(YES_AT), Some(NO_AT)));
        assert!(f.note.contains("under the 10"), "{}", f.note);
        let f = &fit(&[choice_row(&[0.99, 0.01], 0)], 0.95)[0];
        assert_eq!(f.rule.act_at, Some(ACT_AT));
        // Its bands are still reported.
        assert_eq!(f.high.n, 1);
    }

    #[test]
    fn a_written_policy_leaves_out_what_is_not_set() {
        let text =
            serde_json::to_string(&policy(&fit(&[noul_row("q", 0.97, true)], 0.95))).unwrap();
        assert_eq!(text, r#"{"questions":{"q":{"yes_at":0.9,"no_at":0.1}}}"#);
    }
}
