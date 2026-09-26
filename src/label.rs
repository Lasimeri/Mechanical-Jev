//! The same questions over many states: `mjev label`. One request per
//! state, each judged by a policy, one JSON line out per state line in.
//! See label.md.

use std::io::Write;

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::policy::{self, Outcome, Policy};
use crate::protocol::Request;

/// One state read from the input.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Its line in the input, from 1.
    pub line: usize,
    /// The id the line gave, if it gave one.
    pub id: Option<Value>,
    pub state: Value,
}

/// The states of an input: one per non-empty line. A JSON object with a
/// `state` is that state (and its `id`, if any); any other JSON value is
/// the state itself; a line that is not JSON, or every line with
/// `as_text`, is a text state as written.
pub fn items(text: &str, as_text: bool) -> Vec<Item> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            let parsed = (!as_text)
                .then(|| serde_json::from_str::<Value>(l).ok())
                .flatten();
            let (id, state) = match parsed {
                Some(Value::Object(mut o)) if o.contains_key("state") => {
                    let state = o.remove("state").unwrap_or(Value::Null);
                    (o.remove("id"), state)
                }
                Some(v) => (None, v),
                None => (None, Value::String(l.to_string())),
            };
            Item {
                line: i + 1,
                id,
                state,
            }
        })
        .collect()
}

/// The questions of a file: the questions map itself, or a request's
/// `questions`.
pub fn questions_of(v: Value) -> Result<Map<String, Value>, String> {
    match v {
        Value::Object(mut o) if o.get("questions").is_some_and(Value::is_object) => {
            match o.remove("questions") {
                Some(Value::Object(q)) => Ok(q),
                _ => unreachable!(),
            }
        }
        Value::Object(o) => Ok(o),
        _ => Err("expected the questions as a JSON object (id to question)".into()),
    }
}

/// How many states came out each way.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Tally {
    pub act: usize,
    pub review: usize,
    pub escalate: usize,
    pub errors: usize,
}

impl Tally {
    fn add(&mut self, o: Outcome) {
        match o {
            Outcome::Act => self.act += 1,
            Outcome::Review => self.review += 1,
            Outcome::Escalate => self.escalate += 1,
        }
    }
}

/// Ask `questions` about every item, judge each response by `policy`, and
/// write one JSON line per item to `out`: `line`, `id` when given, then
/// the verdict (`outcome`, `answers`, `composites`) or an `error`. A
/// failed item is recorded and the rest go on. `progress` hears (done,
/// total) after each.
pub fn run(
    client: &Client,
    questions: &Map<String, Value>,
    policy: &Policy,
    items: &[Item],
    out: &mut dyn Write,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Tally, String> {
    let asked = policy::asked(&Request {
        model: None,
        state: Value::Null,
        questions: questions.clone(),
    })?;
    policy.check_questions(&asked)?;
    let mut tally = Tally::default();
    for (n, item) in items.iter().enumerate() {
        let req = Request {
            model: None,
            state: item.state.clone(),
            questions: questions.clone(),
        };
        let judged = client
            .system_one(&req)
            .map_err(|e| e.to_string())
            .and_then(|(resp, _)| policy.judge(&asked, &resp));
        let mut rec = Map::new();
        rec.insert("line".into(), json!(item.line));
        if let Some(id) = &item.id {
            rec.insert("id".into(), id.clone());
        }
        match judged {
            Ok(v) => {
                tally.add(v.outcome);
                if let Value::Object(o) = serde_json::to_value(&v).unwrap_or_default() {
                    rec.extend(o);
                }
            }
            Err(e) => {
                tally.errors += 1;
                rec.insert("error".into(), json!(e));
            }
        }
        writeln!(out, "{}", Value::Object(rec)).map_err(|e| e.to_string())?;
        progress(n + 1, items.len());
    }
    Ok(tally)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_come_as_objects_values_or_text() {
        let input = concat!(
            "{\"id\": \"t1\", \"state\": \"I was charged twice.\"}\n",
            "\n",
            "{\"ticket\": \"x\"}\n",
            "Help! My payouts have been failing for 3 days.\n",
            "\"quoted\"\n",
        );
        let it = items(input, false);
        assert_eq!(it.len(), 4);
        assert_eq!(
            (it[0].line, it[0].id.clone(), it[0].state.clone()),
            (1, Some(json!("t1")), json!("I was charged twice."))
        );
        assert_eq!(
            (it[1].line, it[1].state.clone()),
            (3, json!({"ticket": "x"}))
        );
        assert_eq!(
            it[2].state,
            json!("Help! My payouts have been failing for 3 days.")
        );
        assert_eq!(it[3].state, json!("quoted"));
        // As text, every line is itself.
        let t = items(input, true);
        assert_eq!(t[3].state, json!("\"quoted\""));
        assert_eq!(t[0].id, None);
    }

    #[test]
    fn questions_come_bare_or_in_a_request() {
        let q = json!({"u": {"type": "noul", "instructions": "Urgent?"}});
        assert_eq!(
            questions_of(q.clone()).unwrap(),
            q.as_object().unwrap().clone()
        );
        let r = json!({"state": "s", "questions": q});
        assert_eq!(questions_of(r).unwrap(), q.as_object().unwrap().clone());
        assert!(questions_of(json!([1])).is_err());
    }

    #[test]
    fn an_unreachable_server_is_an_error_per_line_and_the_rest_go_on() {
        let client = Client {
            base: "http://127.0.0.1:9".into(),
            api_key: None,
            model: "jev-latest".into(),
            attempts: 1,
            timeout: std::time::Duration::from_secs(1),
        };
        let q = questions_of(json!({"u": {"type": "noul", "instructions": "Urgent?"}})).unwrap();
        let it = items("a\nb\n", true);
        let mut out = Vec::new();
        let mut seen = Vec::new();
        let t = run(
            &client,
            &q,
            &Policy::default(),
            &it,
            &mut out,
            &mut |d, n| seen.push((d, n)),
        )
        .unwrap();
        assert_eq!(t.errors, 2);
        assert_eq!(seen, vec![(1, 2), (2, 2)]);
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[1]["line"], json!(2));
        assert!(lines[1]["error"].is_string());
    }
}
