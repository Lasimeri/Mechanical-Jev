//! What the TUI edits and shows, with no terminal in it: a draft request
//! (the state as text, the questions as editable drafts) to and from the
//! wire `Request`, and an answer as lines of label, probability and note
//! for the screen to draw with bars. See model.md.

use serde_json::{json, Map, Value};

use crate::protocol::{parse_questions, Answer, Question, Request};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Noul,
    Choice,
    Score,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Noul => "noul",
            Kind::Choice => "choice",
            Kind::Score => "score",
        }
    }

    pub fn next(self) -> Kind {
        match self {
            Kind::Noul => Kind::Choice,
            Kind::Choice => Kind::Score,
            Kind::Score => Kind::Noul,
        }
    }

    pub fn prev(self) -> Kind {
        self.next().next()
    }

    /// What the options field holds for this kind.
    pub fn options_hint(self) -> &'static str {
        match self {
            Kind::Noul => "optional: `true: what yes means` and `false: what no means`",
            Kind::Choice => "one option per line: key, or key: description",
            Kind::Score => "one level per line, lowest first (2 to 10)",
        }
    }
}

/// One question as the user edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct QDraft {
    pub id: String,
    pub kind: Kind,
    pub instructions: String,
    /// Choice: `key` or `key: description` per line; Score: a level per line.
    pub options: String,
}

impl QDraft {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            kind: Kind::Noul,
            instructions: String::new(),
            options: String::new(),
        }
    }

    /// The question's wire form; checked by `parse_questions` (the client's
    /// own limits) so an error shows here, not after a round trip.
    pub fn to_value(&self) -> Result<Value, String> {
        let lines = || {
            self.options
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
        };
        let v = match self.kind {
            Kind::Noul => {
                let mut v = json!({"type": "noul", "instructions": structured(&self.instructions)});
                let (mut t, mut f) = (None, None);
                for l in lines() {
                    match l.split_once(':').map(|(k, d)| (k.trim(), d.trim())) {
                        Some(("true", d)) => t = Some(d),
                        Some(("false", d)) => f = Some(d),
                        _ => {
                            return Err(format!(
                                "`{}`: a noul's lines are `true: ...` and `false: ...`",
                                self.id
                            ))
                        }
                    }
                }
                match (t, f) {
                    (Some(t), Some(f)) => {
                        v["criteria"] = json!({"true": structured(t), "false": structured(f)})
                    }
                    (None, None) => {}
                    _ => {
                        return Err(format!(
                            "`{}`: a noul needs both `true:` and `false:`, or neither",
                            self.id
                        ))
                    }
                }
                v
            }
            Kind::Choice => {
                let mut criteria = Map::new();
                for l in lines() {
                    let (k, d) = l.split_once(':').unwrap_or((l, ""));
                    let (k, d) = (k.trim(), d.trim());
                    if criteria.contains_key(k) {
                        return Err(format!("`{}`: option `{k}` twice", self.id));
                    }
                    criteria.insert(
                        k.into(),
                        if d.is_empty() {
                            Value::Null
                        } else {
                            structured(d)
                        },
                    );
                }
                json!({"type": "choice", "instructions": structured(&self.instructions), "criteria": criteria})
            }
            Kind::Score => {
                let levels: Vec<Value> = lines().map(structured).collect();
                json!({"type": "score", "instructions": structured(&self.instructions), "criteria": levels})
            }
        };
        if self.id.trim().is_empty() {
            return Err("a question needs an id".into());
        }
        if self.instructions.trim().is_empty() {
            return Err(format!("`{}`: no instructions", self.id));
        }
        let mut m = Map::new();
        m.insert(self.id.clone(), v.clone());
        parse_questions(&m).map_err(|e| format!("`{}`: {e}", self.id))?;
        Ok(v)
    }

    pub fn from_question(id: &str, q: &Question) -> Self {
        // Instructions have a field of their own lines; an option or a
        // level is one line, so its structure goes compact.
        let instr = |v: &Value| match v {
            Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        let text = |v: &Value| match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        match q {
            Question::Noul {
                instructions,
                criteria,
            } => Self {
                id: id.into(),
                kind: Kind::Noul,
                instructions: instr(instructions),
                options: criteria.as_ref().map_or_else(String::new, |c| {
                    format!("true: {}\nfalse: {}", text(&c.is_true), text(&c.is_false))
                }),
            },
            Question::Choice {
                instructions,
                criteria,
            } => Self {
                id: id.into(),
                kind: Kind::Choice,
                instructions: instr(instructions),
                options: criteria
                    .iter()
                    .map(|(k, v)| match text(v) {
                        d if d.is_empty() => k.clone(),
                        d => format!("{k}: {d}"),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
            Question::Score {
                instructions,
                criteria,
            } => Self {
                id: id.into(),
                kind: Kind::Score,
                instructions: instr(instructions),
                options: criteria.iter().map(text).collect::<Vec<_>>().join("\n"),
            },
        }
    }
}

/// Text as a value: a JSON object or array as that structure (TypeSafe's
/// instructions, descriptions and states may be structured), anything else
/// as the trimmed text.
pub fn structured(s: &str) -> Value {
    let t = s.trim();
    if t.starts_with('{') || t.starts_with('[') {
        if let Ok(v) = serde_json::from_str(t) {
            return v;
        }
    }
    Value::String(t.into())
}

/// A state as it goes on the wire: a JSON object or array as that
/// structure, anything else as the text as written (not trimmed).
pub fn state_value(s: &str) -> Value {
    match structured(s) {
        Value::String(_) => Value::String(s.into()),
        v => v,
    }
}

/// The request being written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Draft {
    pub state: String,
    pub questions: Vec<QDraft>,
}

impl Draft {
    /// The wire request. A state that is a JSON object or array goes as
    /// that structure (Jev reads it indented); anything else as text, as
    /// written.
    pub fn to_request(&self) -> Result<Request, String> {
        if self.questions.is_empty() {
            return Err("no questions yet: add one".into());
        }
        let state = state_value(&self.state);
        let mut questions = Map::new();
        for q in &self.questions {
            if questions.contains_key(&q.id) {
                return Err(format!("id `{}` twice", q.id));
            }
            questions.insert(q.id.clone(), q.to_value()?);
        }
        Ok(Request {
            model: None,
            state,
            questions,
        })
    }

    pub fn from_request(req: &Request) -> Result<Self, String> {
        let state = match &req.state {
            Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        let questions = parse_questions(&req.questions)?
            .iter()
            .map(|(id, q)| QDraft::from_question(id, q))
            .collect();
        Ok(Self { state, questions })
    }

    /// A fresh id: q1, q2, ... not yet used.
    pub fn next_id(&self) -> String {
        (1..)
            .map(|n| format!("q{n}"))
            .find(|id| self.questions.iter().all(|q| &q.id != id))
            .unwrap_or_default()
    }
}

/// One line of an answer: a label, the probability it carries (drawn as a
/// bar), and a note after it.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub label: String,
    pub p: Option<f64>,
    pub note: String,
    /// The chosen option, the answer itself.
    pub chosen: bool,
}

fn line(label: &str, p: Option<f64>, note: String, chosen: bool) -> Line {
    Line {
        label: label.into(),
        p,
        note,
        chosen,
    }
}

fn prob(m: &Map<String, Value>, k: &str) -> f64 {
    m.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

/// The lines of one answer, with Jev's published answer beside it when
/// there is one (an example from TypeSafe's documentation).
pub fn answer_lines(q: &QDraft, answer: &Value, jev: Option<&Value>) -> Vec<Line> {
    let ours: Option<Answer> = serde_json::from_value(answer.clone()).ok();
    let theirs: Option<Answer> = jev.and_then(|j| serde_json::from_value(j.clone()).ok());
    let jev_note = |s: String| {
        if theirs.is_some() {
            format!("jev {s}")
        } else {
            String::new()
        }
    };
    match ours {
        None => vec![line("no answer", None, answer.to_string(), false)],
        Some(Answer::Noul { noul }) => {
            let j = match &theirs {
                Some(Answer::Noul { noul: t }) => jev_note(format!("{t:.2}")),
                _ => String::new(),
            };
            vec![line("p(yes)", Some(noul), j, noul >= 0.5)]
        }
        Some(Answer::Choice {
            choice,
            probabilities,
            confidence,
        }) => {
            let jp = match &theirs {
                Some(Answer::Choice { probabilities, .. }) => Some(probabilities.clone()),
                _ => None,
            };
            let keys: Vec<String> = q
                .options
                .lines()
                .map(|l| l.split_once(':').map_or(l, |(k, _)| k).trim().to_string())
                .filter(|k| !k.is_empty())
                .collect();
            let mut out: Vec<Line> = keys
                .iter()
                .map(|k| {
                    let note = jp
                        .as_ref()
                        .map(|m| format!("jev {:.2}", prob(m, k)))
                        .unwrap_or_default();
                    line(k, Some(prob(&probabilities, k)), note, *k == choice)
                })
                .collect();
            let jc = match &theirs {
                Some(Answer::Choice { confidence, .. }) => jev_note(format!("{confidence:.2}")),
                _ => String::new(),
            };
            out.push(line("confidence", Some(confidence), jc, false));
            out
        }
        Some(Answer::Score {
            score,
            probabilities,
            confidence,
            ..
        }) => {
            let jp = match &theirs {
                Some(Answer::Score { probabilities, .. }) => Some(probabilities.clone()),
                _ => None,
            };
            let levels: Vec<&str> = q
                .options
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            let mut out: Vec<Line> = levels
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let k = i.to_string();
                    let note = jp
                        .as_ref()
                        .map(|m| format!("jev {:.2}", prob(m, &k)))
                        .unwrap_or_default();
                    line(
                        &format!("{i} {name}"),
                        Some(prob(&probabilities, &k)),
                        note,
                        score.round() as usize == i,
                    )
                })
                .collect();
            let js = match &theirs {
                Some(Answer::Score {
                    score: s,
                    confidence: c,
                    ..
                }) => jev_note(format!("{s:.2}, {c:.2}")),
                _ => String::new(),
            };
            out.push(line(
                &format!("score {score:.2}, confidence"),
                Some(confidence),
                js,
                false,
            ));
            out
        }
    }
}

/// A probability as `width` cells: how many are filled.
pub fn filled(p: f64, width: usize) -> usize {
    ((p.clamp(0.0, 1.0) * width as f64).round() as usize).min(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_round_trips_through_the_wire_form() {
        let mut d = Draft {
            state: "$ git status".into(),
            questions: vec![QDraft::new("ro")],
        };
        d.questions[0].instructions = "Is this command read-only?".into();
        let mut c = QDraft::new("kind");
        c.kind = Kind::Choice;
        c.instructions = "What kind?".into();
        c.options = "vcs: version control\nbuild\n".into();
        d.questions.push(c);
        let req = d.to_request().unwrap();
        assert_eq!(req.state, json!("$ git status"));
        assert_eq!(
            req.questions["kind"]["criteria"],
            json!({"vcs": "version control", "build": null})
        );
        assert_eq!(
            Draft::from_request(&req).unwrap().questions[1].options,
            "vcs: version control\nbuild"
        );
    }

    #[test]
    fn a_json_state_goes_as_structure() {
        let mut d = Draft {
            state: "{\"a\": 1}".into(),
            questions: vec![QDraft::new("q")],
        };
        d.questions[0].instructions = "?".into();
        assert_eq!(d.to_request().unwrap().state, json!({"a": 1}));
    }

    #[test]
    fn a_bad_question_is_caught_before_sending() {
        let mut q = QDraft::new("s");
        q.kind = Kind::Score;
        q.instructions = "rate".into();
        q.options = "only one level".into();
        assert!(q.to_value().is_err());
        q.options = "low\nhigh".into();
        assert!(q.to_value().is_ok());
    }

    #[test]
    fn answers_become_lines_with_jev_beside_them() {
        let mut q = QDraft::new("d");
        q.kind = Kind::Choice;
        q.options = "billing\ntechnical".into();
        let ours = json!({"type": "choice", "choice": "billing", "probabilities": {"billing": 0.7, "technical": 0.3}, "confidence": 0.4});
        let jev = json!({"type": "choice", "choice": "billing", "probabilities": {"billing": 0.88, "technical": 0.12}, "confidence": 0.76});
        let l = answer_lines(&q, &ours, Some(&jev));
        assert_eq!(l.len(), 3);
        assert!(l[0].chosen && !l[1].chosen);
        assert_eq!(l[0].note, "jev 0.88");
        assert_eq!(l[2].note, "jev 0.76");
        assert_eq!(filled(0.5, 10), 5);
        assert_eq!(filled(1.7, 10), 10);
    }

    #[test]
    fn every_published_request_survives_the_editor() {
        for e in crate::evidence::all() {
            let req: Request = serde_json::from_value(e["request"].clone()).unwrap();
            let back = Draft::from_request(&req).unwrap().to_request().unwrap();
            assert_eq!(back, req, "{}", e["request"]);
        }
    }
}
