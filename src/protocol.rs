//! The wire format of TypeSafe's `POST /v1/systemone`, field for field
//! (docs.typesafe.ai/api), and a builder for requests. See protocol.md.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The most options a Choice may have, per TypeSafe.
pub const MAX_OPTIONS: usize = 255;
/// A Score's levels, per TypeSafe.
pub const MIN_LEVELS: usize = 2;
pub const MAX_LEVELS: usize = 10;

/// A request: the state and named typed questions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Request {
    /// The model; `jev-latest` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// A string, or any JSON object or array.
    pub state: Value,
    /// Question id to question. Ids are never shown to the model.
    pub questions: Map<String, Value>,
}

/// One typed question.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Is this statement true? Answered with P(yes).
    Noul {
        instructions: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// One option of a set; each option's description may be null.
    Choice {
        instructions: Value,
        criteria: Map<String, Value>,
    },
    /// An ordered rubric, lowest level first.
    Score {
        instructions: Value,
        criteria: Vec<Value>,
    },
}

/// What a Noul's yes and no mean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub is_true: Value,
    #[serde(rename = "false")]
    pub is_false: Value,
}

/// One typed answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Noul {
        /// P(yes), 0 to 1.
        noul: f64,
    },
    Choice {
        choice: String,
        probabilities: Map<String, Value>,
        confidence: f64,
    },
    Score {
        /// The probability-weighted level; can land between levels.
        score: f64,
        #[serde(default)]
        legend: Map<String, Value>,
        probabilities: Map<String, Value>,
        confidence: f64,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// The response: one answer per question id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub model: String,
    pub answers: Map<String, Value>,
    #[serde(default)]
    pub usage: Usage,
}

impl Response {
    /// The typed answer to `id`.
    pub fn answer(&self, id: &str) -> Option<Answer> {
        serde_json::from_value(self.answers.get(id)?.clone()).ok()
    }

    /// P(yes) of a Noul.
    pub fn noul(&self, id: &str) -> Option<f64> {
        match self.answer(id)? {
            Answer::Noul { noul } => Some(noul),
            _ => None,
        }
    }

    /// The chosen option and its confidence.
    pub fn choice(&self, id: &str) -> Option<(String, f64)> {
        match self.answer(id)? {
            Answer::Choice {
                choice, confidence, ..
            } => Some((choice, confidence)),
            _ => None,
        }
    }

    /// The score and its confidence.
    pub fn score(&self, id: &str) -> Option<(f64, f64)> {
        match self.answer(id)? {
            Answer::Score {
                score, confidence, ..
            } => Some((score, confidence)),
            _ => None,
        }
    }
}

/// The questions of a request, parsed and checked against TypeSafe's
/// limits, in request order.
pub fn parse_questions(raw: &Map<String, Value>) -> Result<Vec<(String, Question)>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for (id, v) in raw {
        let q: Question =
            serde_json::from_value(v.clone()).map_err(|e| format!("question `{id}`: {e}"))?;
        match &q {
            Question::Choice { criteria, .. } if criteria.is_empty() => {
                return Err(format!(
                    "question `{id}`: a choice needs at least one option"
                ))
            }
            Question::Choice { criteria, .. } if criteria.len() > MAX_OPTIONS => {
                return Err(format!(
                    "question `{id}`: {} options; a choice takes at most {MAX_OPTIONS}",
                    criteria.len()
                ))
            }
            Question::Score { criteria, .. }
                if criteria.len() < MIN_LEVELS || criteria.len() > MAX_LEVELS =>
            {
                return Err(format!(
                    "question `{id}`: a score takes {MIN_LEVELS} to {MAX_LEVELS} levels"
                ))
            }
            _ => {}
        }
        out.push((id.clone(), q));
    }
    Ok(out)
}

/// A request built in code:
///
/// ```
/// use mechanical_jev::protocol::RequestBuilder;
/// let req = RequestBuilder::new("Help! My payouts have been failing for 3 days.")
///     .noul("is_urgent", "Does this convey urgency?")
///     .choice("department", "Which team should handle this?",
///             &[("billing", "Payments, invoicing, refunds"), ("technical", "Bugs, outages, integrations")])
///     .score("frustration", "How frustrated is the customer?", &["Calm", "Frustrated", "Very angry"])
///     .build()
///     .unwrap();
/// assert_eq!(req.questions.len(), 3);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RequestBuilder {
    req: Request,
}

impl RequestBuilder {
    /// A request about `state`: a string, or anything `serde_json` can turn
    /// into a JSON value.
    pub fn new(state: impl Serialize) -> Self {
        Self {
            req: Request {
                model: None,
                state: serde_json::to_value(state).unwrap_or(Value::Null),
                questions: Map::new(),
            },
        }
    }

    pub fn model(mut self, model: &str) -> Self {
        self.req.model = Some(model.to_string());
        self
    }

    pub fn noul(mut self, id: &str, instructions: &str) -> Self {
        self.req.questions.insert(
            id.into(),
            serde_json::json!({"type": "noul", "instructions": instructions}),
        );
        self
    }

    pub fn choice(mut self, id: &str, instructions: &str, options: &[(&str, &str)]) -> Self {
        let criteria: Map<String, Value> = options
            .iter()
            .map(|(k, d)| {
                let d = if d.is_empty() {
                    Value::Null
                } else {
                    Value::from(*d)
                };
                ((*k).to_string(), d)
            })
            .collect();
        self.req.questions.insert(
            id.into(),
            serde_json::json!({"type": "choice", "instructions": instructions, "criteria": criteria}),
        );
        self
    }

    pub fn score(mut self, id: &str, instructions: &str, levels: &[&str]) -> Self {
        self.req.questions.insert(
            id.into(),
            serde_json::json!({"type": "score", "instructions": instructions, "criteria": levels}),
        );
        self
    }

    /// The request, checked against TypeSafe's limits.
    pub fn build(self) -> Result<Request, String> {
        parse_questions(&self.req.questions)?;
        Ok(self.req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The response examples of docs.typesafe.ai/api, verbatim.
    #[test]
    fn parses_the_documented_responses() {
        let r: Response = serde_json::from_value(json!({
            "model": "jev-1.13.0",
            "answers": {
                "is_urgent": {"type": "noul", "noul": 0.95},
                "department": {"type": "choice", "choice": "billing",
                    "probabilities": {"billing": 0.88, "technical": 0.12, "sales": 0.0},
                    "confidence": 0.81},
                "frustration": {"type": "score", "score": 1.05,
                    "legend": {"0": "Calm", "1": "Frustrated", "2": "Very angry"},
                    "probabilities": {"0": 0.0, "1": 0.95, "2": 0.05}, "confidence": 0.92}
            },
            "usage": {"input_tokens": 318, "output_tokens": 34}
        }))
        .unwrap();
        assert_eq!(r.noul("is_urgent"), Some(0.95));
        assert_eq!(r.choice("department"), Some(("billing".into(), 0.81)));
        assert_eq!(r.score("frustration"), Some((1.05, 0.92)));
        assert_eq!(r.usage.input_tokens, 318);
    }

    #[test]
    fn limits_are_checked() {
        let many: Vec<(String, String)> =
            (0..256).map(|i| (format!("o{i}"), String::new())).collect();
        let refs: Vec<(&str, &str)> = many.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        assert!(RequestBuilder::new("s")
            .choice("c", "?", &refs)
            .build()
            .is_err());
        assert!(RequestBuilder::new("s")
            .score("s", "?", &["one"])
            .build()
            .is_err());
        assert!(RequestBuilder::new("s")
            .score("s", "?", &["a", "b"])
            .build()
            .is_ok());
    }
}
