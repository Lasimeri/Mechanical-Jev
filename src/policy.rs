//! What to do with an answer: act, review or escalate. The answer says
//! what; its probability or confidence says whether to act on it, and
//! the thresholds are the caller's, kept here in code and in a policy
//! file rather than in the model. See policy.md.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::protocol::{Answer, Response};

/// What the caller should do with an answer, worst last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Confident enough to act without anyone looking.
    Act,
    /// A reasonable answer, not a sure one: confirm, or have it checked.
    Review,
    /// The model is unsure: do not act on it; a person or another system.
    Escalate,
}

impl Outcome {
    /// The process exit code `mjev gate` ends with: kept apart from 1
    /// (an error) and 2 (a usage mistake), so a failed call never reads
    /// as a decision.
    pub fn exit_code(self) -> i32 {
        match self {
            Outcome::Act => 0,
            Outcome::Review => 10,
            Outcome::Escalate => 11,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Outcome::Act => "act",
            Outcome::Review => "review",
            Outcome::Escalate => "escalate",
        }
    }
}

/// Thresholds for one question, or for every question of the policy
/// (`defaults`). A field left out falls back to the defaults, then to
/// the built-in value.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Noul: at or above, a confident yes (0.9).
    pub yes_at: Option<f64>,
    /// Noul: at or below, a confident no (0.1).
    pub no_at: Option<f64>,
    /// Choice and Score: confidence below this escalates (0.5).
    pub floor: Option<f64>,
    /// Choice and Score: confidence at or above this acts (0.9).
    pub act_at: Option<f64>,
    /// Choice: a stricter (or looser) `act_at` for one option: an
    /// irreversible option wants more confidence than a read-only one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub act_at_option: BTreeMap<String, f64>,
    /// Score: a threshold on the score itself; the decision is then
    /// `above` (at or above it) or `below`.
    pub above: Option<f64>,
    /// An answer asked speculatively: reported, never deciding.
    #[serde(default)]
    pub ignore: bool,
}

/// Several answers combined with weights into one number from 0 to 1:
/// a Score as its level over the top level, a Noul as its probability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Composite {
    /// Question id to weight.
    pub weights: BTreeMap<String, f64>,
    /// At or above: act. Without it the composite only ranks.
    pub act_above: Option<f64>,
    /// At or above (and under `act_above`): review; under it: escalate.
    pub review_above: Option<f64>,
}

/// A policy file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    pub defaults: Rule,
    #[serde(default)]
    pub questions: BTreeMap<String, Rule>,
    #[serde(default)]
    pub composites: BTreeMap<String, Composite>,
}

/// The built-in thresholds: TypeSafe's worked examples (docs.typesafe.ai
/// confidence.md: 0.5 as the floor for genuine uncertainty, 0.9 for
/// acting without confirmation; a Noul acted on at 0.9 or 0.1). Starting
/// points to replace with ones fitted on the caller's own data.
pub const YES_AT: f64 = 0.9;
pub const NO_AT: f64 = 0.1;
pub const FLOOR: f64 = 0.5;
pub const ACT_AT: f64 = 0.9;

/// One answer judged.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Judged {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// P(yes), the chosen option, or the score.
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// yes / no / undecided; the option; above / below or the level.
    pub decision: String,
    pub outcome: Outcome,
    /// The threshold the outcome was measured against, said plainly.
    pub why: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
}

/// A composite judged, its dimensions kept: a composite of 0.82 alone
/// cannot say which dimension made it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CompositeJudged {
    pub id: String,
    pub value: f64,
    /// Question id to (raw answer, the 0 to 1 it counted as, weight).
    pub dimensions: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
}

/// Everything a policy made of one response.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Verdict {
    pub answers: Vec<Judged>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub composites: Vec<CompositeJudged>,
    /// The worst outcome of every deciding answer and composite.
    pub outcome: Outcome,
}

fn check01(what: &str, v: Option<f64>) -> Result<(), String> {
    match v {
        Some(x) if !(0.0..=1.0).contains(&x) => Err(format!("{what} is {x}; it must be 0 to 1")),
        _ => Ok(()),
    }
}

impl Rule {
    fn check(&self, at: &str) -> Result<(), String> {
        for (k, v) in [
            ("yes_at", self.yes_at),
            ("no_at", self.no_at),
            ("floor", self.floor),
            ("act_at", self.act_at),
        ] {
            check01(&format!("{at}.{k}"), v)?;
        }
        for (o, v) in &self.act_at_option {
            check01(&format!("{at}.act_at_option.{o}"), Some(*v))?;
        }
        Ok(())
    }

    /// This rule over `base`: each field set here wins.
    fn over(&self, base: &Rule) -> Rule {
        let mut act_at_option = base.act_at_option.clone();
        act_at_option.extend(self.act_at_option.clone());
        Rule {
            yes_at: self.yes_at.or(base.yes_at),
            no_at: self.no_at.or(base.no_at),
            floor: self.floor.or(base.floor),
            act_at: self.act_at.or(base.act_at),
            act_at_option,
            above: self.above.or(base.above),
            ignore: self.ignore || base.ignore,
        }
    }
}

impl Policy {
    /// A policy file read and checked.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let p: Policy =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        p.check().map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(p)
    }

    /// Thresholds in range and in order, composites with positive
    /// weights and thresholds in order.
    pub fn check(&self) -> Result<(), String> {
        self.defaults.check("defaults")?;
        for (id, r) in &self.questions {
            r.check(&format!("questions.{id}"))?;
        }
        for (id, r) in std::iter::once(("defaults", &self.defaults))
            .chain(self.questions.iter().map(|(k, v)| (k.as_str(), v)))
        {
            let r = r.over(&self.defaults);
            let (yes, no) = (r.yes_at.unwrap_or(YES_AT), r.no_at.unwrap_or(NO_AT));
            if no >= yes {
                return Err(format!("{id}: no_at ({no}) must be under yes_at ({yes})"));
            }
            let (floor, act) = (r.floor.unwrap_or(FLOOR), r.act_at.unwrap_or(ACT_AT));
            if floor > act {
                return Err(format!(
                    "{id}: floor ({floor}) must not be over act_at ({act})"
                ));
            }
        }
        for (id, c) in &self.composites {
            if c.weights.is_empty() {
                return Err(format!("composites.{id}: no weights"));
            }
            if let Some((q, w)) = c.weights.iter().find(|(_, w)| w.is_nan() || **w <= 0.0) {
                return Err(format!(
                    "composites.{id}.weights.{q} is {w}; weights are positive"
                ));
            }
            check01(&format!("composites.{id}.act_above"), c.act_above)?;
            check01(&format!("composites.{id}.review_above"), c.review_above)?;
            if let (Some(a), Some(r)) = (c.act_above, c.review_above) {
                if r > a {
                    return Err(format!(
                        "composites.{id}: review_above ({r}) must not be over act_above ({a})"
                    ));
                }
            }
        }
        Ok(())
    }

    /// The rule for question `id`.
    pub fn rule(&self, id: &str) -> Rule {
        match self.questions.get(id) {
            Some(r) => r.over(&self.defaults),
            None => self.defaults.clone(),
        }
    }

    /// The policy against what a request asks, before anything is sent: a
    /// question it names that is not asked is a typo, and a composite
    /// cannot weigh a Choice.
    pub fn check_questions(&self, asked: &[(String, String)]) -> Result<(), String> {
        let kind = |id: &str| asked.iter().find(|(q, _)| q == id).map(|(_, k)| k.as_str());
        for id in self.questions.keys() {
            if kind(id).is_none() {
                return Err(format!(
                    "the policy names `{id}`, which the request does not ask"
                ));
            }
        }
        for (cid, c) in &self.composites {
            for q in c.weights.keys() {
                match kind(q) {
                    None => {
                        return Err(format!(
                            "composite `{cid}` names `{q}`, which the request does not ask"
                        ))
                    }
                    Some("choice") => {
                        return Err(format!(
                            "composite `{cid}` names `{q}`, a Choice: its options have no order to weigh"
                        ))
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(())
    }

    /// Judge a response. `asked` is the request's question ids with their
    /// types, in order (`asked`).
    pub fn judge(&self, asked: &[(String, String)], resp: &Response) -> Result<Verdict, String> {
        self.check_questions(asked)?;
        let mut answers = Vec::with_capacity(asked.len());
        for (id, _) in asked {
            let a = resp
                .answer(id)
                .ok_or_else(|| format!("the response has no answer for `{id}`"))?;
            answers.push(judge_one(id, &a, &self.rule(id)));
        }
        let mut composites = Vec::new();
        for (cid, c) in &self.composites {
            composites.push(composite(cid, c, resp)?);
        }
        let outcome = answers
            .iter()
            .filter(|j| !j.ignored)
            .map(|j| j.outcome)
            .chain(composites.iter().filter_map(|c| c.outcome))
            .max()
            .unwrap_or(Outcome::Act);
        Ok(Verdict {
            answers,
            composites,
            outcome,
        })
    }
}

/// A request's question ids and types, in request order.
pub fn asked(req: &crate::protocol::Request) -> Result<Vec<(String, String)>, String> {
    use crate::protocol::Question;
    Ok(crate::protocol::parse_questions(&req.questions)?
        .into_iter()
        .map(|(id, q)| {
            let kind = match q {
                Question::Noul { .. } => "noul",
                Question::Choice { .. } => "choice",
                Question::Score { .. } => "score",
            };
            (id, kind.to_string())
        })
        .collect())
}

fn f(x: f64) -> String {
    format!("{x:.2}")
}

/// One answer against its rule.
pub fn judge_one(id: &str, a: &Answer, r: &Rule) -> Judged {
    let floor = r.floor.unwrap_or(FLOOR);
    // Choice and Score: under the floor escalate, at `act` act, between review.
    let band = |c: f64, act: f64| {
        if c < floor {
            (
                Outcome::Escalate,
                format!("confidence {} under the floor {}", f(c), f(floor)),
            )
        } else if c >= act {
            (
                Outcome::Act,
                format!("confidence {} at or over {}", f(c), f(act)),
            )
        } else {
            (
                Outcome::Review,
                format!("confidence {} under {} to act", f(c), f(act)),
            )
        }
    };
    let (kind, value, confidence, decision, (outcome, why)) = match a {
        Answer::Noul { noul } => {
            let (yes, no) = (r.yes_at.unwrap_or(YES_AT), r.no_at.unwrap_or(NO_AT));
            let (decision, judged) = if *noul >= yes {
                (
                    "yes",
                    (
                        Outcome::Act,
                        format!("p(yes) {} at or over {}", f(*noul), f(yes)),
                    ),
                )
            } else if *noul <= no {
                (
                    "no",
                    (
                        Outcome::Act,
                        format!("p(yes) {} at or under {}", f(*noul), f(no)),
                    ),
                )
            } else {
                (
                    "undecided",
                    (
                        Outcome::Review,
                        format!("p(yes) {} between {} and {}", f(*noul), f(no), f(yes)),
                    ),
                )
            };
            ("noul", json!(noul), None, decision.to_string(), judged)
        }
        Answer::Choice {
            choice, confidence, ..
        } => {
            let act = r
                .act_at_option
                .get(choice)
                .copied()
                .or(r.act_at)
                .unwrap_or(ACT_AT);
            (
                "choice",
                json!(choice),
                Some(*confidence),
                choice.clone(),
                band(*confidence, act),
            )
        }
        Answer::Score {
            score,
            legend,
            probabilities,
            confidence,
        } => {
            let decision = match r.above {
                Some(t) if *score >= t => format!("above {}", f(t)),
                Some(t) => format!("below {}", f(t)),
                None => {
                    let top = levels(probabilities, legend).saturating_sub(1);
                    let level = (score.round().max(0.0) as usize).min(top);
                    match legend.get(&level.to_string()).and_then(Value::as_str) {
                        Some(l) => format!("level {level}: {l}"),
                        None => format!("level {level}"),
                    }
                }
            };
            (
                "score",
                json!(score),
                Some(*confidence),
                decision,
                band(*confidence, r.act_at.unwrap_or(ACT_AT)),
            )
        }
    };
    Judged {
        id: id.to_string(),
        kind: kind.to_string(),
        value,
        confidence,
        decision,
        outcome,
        why,
        ignored: r.ignore,
    }
}

/// How many levels a Score answer has: its probabilities, else its legend.
fn levels(probabilities: &Map<String, Value>, legend: &Map<String, Value>) -> usize {
    probabilities.len().max(legend.len())
}

/// A composite: each dimension from 0 to 1, weighted, averaged.
fn composite(id: &str, c: &Composite, resp: &Response) -> Result<CompositeJudged, String> {
    let mut dims = BTreeMap::new();
    let (mut sum, mut total) = (0.0, 0.0);
    for (q, w) in &c.weights {
        let a = resp
            .answer(q)
            .ok_or_else(|| format!("composite `{id}` names `{q}`, which has no answer"))?;
        let (raw, unit) = match &a {
            Answer::Noul { noul } => (json!(noul), *noul),
            Answer::Score {
                score,
                legend,
                probabilities,
                ..
            } => {
                let top = levels(probabilities, legend).saturating_sub(1).max(1);
                (json!(score), score / top as f64)
            }
            Answer::Choice { .. } => {
                return Err(format!(
                    "composite `{id}` names `{q}`, a Choice: its options have no order to weigh"
                ))
            }
        };
        dims.insert(q.clone(), json!({"raw": raw, "unit": unit, "weight": w}));
        sum += w * unit;
        total += w;
    }
    let value = sum / total;
    let outcome = c.act_above.map(|a| {
        if value >= a {
            Outcome::Act
        } else if c.review_above.is_some_and(|r| value >= r) {
            Outcome::Review
        } else {
            Outcome::Escalate
        }
    });
    Ok(CompositeJudged {
        id: id.to_string(),
        value,
        dimensions: dims,
        outcome,
    })
}

/// A verdict as lines a person reads: one per answer and composite, then
/// the overall outcome.
pub fn text(v: &Verdict) -> String {
    let w = v
        .answers
        .iter()
        .map(|j| j.id.chars().count())
        .chain(v.composites.iter().map(|c| c.id.chars().count()))
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for j in &v.answers {
        let value = match &j.value {
            Value::String(s) => s.clone(),
            Value::Number(n) => format!("{:.2}", n.as_f64().unwrap_or(0.0)),
            other => other.to_string(),
        };
        let tail = if j.ignored {
            "ignored (asked speculatively)".to_string()
        } else {
            format!("{}: {}", j.outcome.name(), j.why)
        };
        let decision = if j.kind == "choice" {
            String::new()
        } else {
            format!("  {}", j.decision)
        };
        out.push_str(&format!(
            "{:<w$}  {:<6}  {value}{decision}  {tail}\n",
            j.id, j.kind
        ));
    }
    for c in &v.composites {
        let dims: Vec<String> = c
            .dimensions
            .iter()
            .map(|(q, d)| {
                format!(
                    "{q} {:.2}x{}",
                    d["unit"].as_f64().unwrap_or(0.0),
                    d["weight"]
                )
            })
            .collect();
        let tail = c
            .outcome
            .map_or("ranks only".to_string(), |o| o.name().to_string());
        out.push_str(&format!(
            "{:<w$}  composite  {:.2} ({})  {tail}\n",
            c.id,
            c.value,
            dims.join(" + ")
        ));
    }
    out.push_str(&format!("outcome: {}\n", v.outcome.name()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(answers: Value) -> Response {
        serde_json::from_value(json!({"model": "m", "answers": answers})).unwrap()
    }

    fn asked(ids: &[(&str, &str)]) -> Vec<(String, String)> {
        ids.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn a_noul_acts_at_either_end_and_is_reviewed_between() {
        let p = Policy::default();
        let q = asked(&[("u", "noul")]);
        let v = |x: f64| {
            p.judge(&q, &resp(json!({"u": {"type": "noul", "noul": x}})))
                .unwrap()
        };
        assert_eq!(
            (v(0.95).outcome, v(0.95).answers[0].decision.as_str()),
            (Outcome::Act, "yes")
        );
        assert_eq!(
            (v(0.05).outcome, v(0.05).answers[0].decision.as_str()),
            (Outcome::Act, "no")
        );
        assert_eq!(v(0.5).outcome, Outcome::Review);
        assert_eq!(v(0.9).outcome, Outcome::Act, "the threshold itself acts");
    }

    #[test]
    fn a_choice_escalates_under_the_floor_and_a_risky_option_needs_more() {
        // TypeSafe's voice banking example: 0.6 floor, 0.85 to approve.
        let p: Policy = serde_json::from_value(json!({
            "defaults": {"floor": 0.6, "act_at": 0.6},
            "questions": {"intent": {"act_at_option": {"approve_transfer": 0.85}}}
        }))
        .unwrap();
        p.check().unwrap();
        let q = asked(&[("intent", "choice")]);
        let v = |choice: &str, c: f64| {
            p.judge(
                &q,
                &resp(json!({"intent": {"type": "choice", "choice": choice,
                    "probabilities": {}, "confidence": c}})),
            )
            .unwrap()
            .outcome
        };
        assert_eq!(v("check_balance", 0.7), Outcome::Act);
        assert_eq!(v("approve_transfer", 0.7), Outcome::Review);
        assert_eq!(v("approve_transfer", 0.9), Outcome::Act);
        assert_eq!(v("check_balance", 0.4), Outcome::Escalate);
    }

    #[test]
    fn a_score_threshold_is_on_the_expectation_and_its_level_is_named() {
        let p: Policy =
            serde_json::from_value(json!({"questions": {"f": {"above": 1.5}}})).unwrap();
        let q = asked(&[("f", "score")]);
        let a = |s: f64| {
            json!({"f": {"type": "score", "score": s, "confidence": 0.95,
                "legend": {"0": "calm", "1": "frustrated", "2": "angry"},
                "probabilities": {"0": 0.1, "1": 0.2, "2": 0.7}}})
        };
        assert_eq!(
            p.judge(&q, &resp(a(1.6))).unwrap().answers[0].decision,
            "above 1.50"
        );
        assert_eq!(
            p.judge(&q, &resp(a(1.2))).unwrap().answers[0].decision,
            "below 1.50"
        );
        let plain = Policy::default().judge(&q, &resp(a(1.2))).unwrap();
        assert_eq!(plain.answers[0].decision, "level 1: frustrated");
    }

    #[test]
    fn the_worst_deciding_answer_decides_and_a_speculative_one_does_not() {
        let q = asked(&[("a", "noul"), ("b", "noul")]);
        let r = resp(json!({"a": {"type": "noul", "noul": 0.99},
                            "b": {"type": "noul", "noul": 0.5}}));
        assert_eq!(
            Policy::default().judge(&q, &r).unwrap().outcome,
            Outcome::Review
        );
        let p: Policy =
            serde_json::from_value(json!({"questions": {"b": {"ignore": true}}})).unwrap();
        let v = p.judge(&q, &r).unwrap();
        assert_eq!(v.outcome, Outcome::Act);
        assert!(v.answers[1].ignored);
    }

    #[test]
    fn a_composite_weighs_units_and_keeps_its_dimensions() {
        // TypeSafe's composite scoring: weights over scores normalised to 0..1.
        let p: Policy = serde_json::from_value(json!({"composites": {"ic": {
            "weights": {"py": 0.4, "design": 0.4, "lead": 0.2},
            "act_above": 0.7, "review_above": 0.4}}}))
        .unwrap();
        p.check().unwrap();
        let sc = |s: f64| {
            json!({"type": "score", "score": s, "confidence": 0.95, "legend": {},
                "probabilities": {"0": 0, "1": 0, "2": 0, "3": 0, "4": 1}})
        };
        let r = resp(json!({"py": sc(4.0), "design": sc(3.0), "lead": sc(1.0)}));
        let q = asked(&[("py", "score"), ("design", "score"), ("lead", "score")]);
        let v = p.judge(&q, &r).unwrap();
        let c = &v.composites[0];
        // (0.4*1.0 + 0.4*0.75 + 0.2*0.25) / 1.0 = 0.75
        assert!((c.value - 0.75).abs() < 1e-9, "{}", c.value);
        assert_eq!(c.outcome, Some(Outcome::Act));
        assert_eq!(c.dimensions["design"]["unit"], json!(0.75));
    }

    #[test]
    fn mistakes_in_a_policy_are_said_before_any_answer() {
        let bad = |v: Value| serde_json::from_value::<Policy>(v).map_err(|e| e.to_string());
        assert!(
            bad(json!({"defaults": {"yes": 0.9}})).is_err(),
            "unknown field"
        );
        let p = bad(json!({"defaults": {"yes_at": 0.2, "no_at": 0.3}})).unwrap();
        assert!(p.check().unwrap_err().contains("no_at"));
        let p = bad(json!({"defaults": {"floor": 1.5}})).unwrap();
        assert!(p.check().unwrap_err().contains("0 to 1"));
        let p = bad(json!({"questions": {"x": {"ignore": true}}})).unwrap();
        let err = p
            .judge(
                &asked(&[("y", "noul")]),
                &resp(json!({"y": {"type": "noul", "noul": 0.5}})),
            )
            .unwrap_err();
        assert!(err.contains("`x`"), "{err}");
        let c = bad(json!({"composites": {"c": {"weights": {"k": 1}}}})).unwrap();
        let err = c
            .judge(
                &asked(&[("k", "choice")]),
                &resp(json!({"k": {"type": "choice", "choice": "a",
                    "probabilities": {}, "confidence": 1.0}})),
            )
            .unwrap_err();
        assert!(err.contains("no order"), "{err}");
    }

    #[test]
    fn exit_codes_never_collide_with_errors() {
        let codes: Vec<i32> = [Outcome::Act, Outcome::Review, Outcome::Escalate]
            .iter()
            .map(|o| o.exit_code())
            .collect();
        assert_eq!(codes, vec![0, 10, 11]);
        assert!(!codes.contains(&1) && !codes.contains(&2));
    }
}
