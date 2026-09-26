//! Rank candidates against a query: `mjev rank`. One Noul per candidate,
//! TypeSafe's re-ranking shape (docs.typesafe.ai, cookbooks/rerank):
//! the probability of yes is the score, sorted in code. See rank.md.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::protocol::Request;

/// The question each candidate is asked unless the caller words it.
pub const INSTRUCTIONS: &str = "Does the candidate answer `query`?";
/// What yes and no mean, as the re-ranking cookbook defines them for its
/// question.
pub const TRUE: &str = "The candidate contains what `query` asks for.";
pub const FALSE: &str = "The candidate is about something else, or only shares words with `query`.";

/// Candidates per request: two rounds of `xks`'s 15 forks, and a request
/// well under its token limits for candidates of a paragraph each.
pub const CHUNK: usize = 30;
/// And no more candidate text than this per request.
pub const CHUNK_CHARS: usize = 24_000;

/// One candidate ranked.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ranked {
    /// Its place in the input, from 1.
    pub index: usize,
    pub text: String,
    /// P(yes): how surely it answers the query.
    pub p: f64,
}

/// The requests that ask about `candidates`, in order: the query as the
/// state (read once per request, every question forked from it on
/// `xks`), each candidate in its own Noul (`c1`, `c2`, ... by input
/// place), cut to `CHUNK` candidates or `CHUNK_CHARS` of their text.
pub fn requests(query: &str, candidates: &[String], instructions: &str) -> Vec<Request> {
    let mut out = Vec::new();
    let mut questions = Map::new();
    let mut chars = 0;
    for (i, c) in candidates.iter().enumerate() {
        if !questions.is_empty()
            && (questions.len() == CHUNK || chars + c.chars().count() > CHUNK_CHARS)
        {
            out.push(request(query, std::mem::take(&mut questions)));
            chars = 0;
        }
        chars += c.chars().count();
        questions.insert(
            format!("c{}", i + 1),
            json!({
                "type": "noul",
                "instructions": format!("{instructions}\n\ncandidate: {c}"),
                "criteria": {"true": TRUE, "false": FALSE},
            }),
        );
    }
    if !questions.is_empty() {
        out.push(request(query, questions));
    }
    out
}

fn request(query: &str, questions: Map<String, Value>) -> Request {
    Request {
        model: None,
        state: json!({ "query": query }),
        questions,
    }
}

/// Ask, collect every candidate's P(yes), sort surest first (input order
/// among equals).
pub fn rank(
    client: &Client,
    query: &str,
    candidates: &[String],
    instructions: &str,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Vec<Ranked>, String> {
    let reqs = requests(query, candidates, instructions);
    let mut p = vec![None; candidates.len()];
    for (n, req) in reqs.iter().enumerate() {
        let (resp, _) = client.system_one(req).map_err(|e| e.to_string())?;
        for id in req.questions.keys() {
            let i: usize = id[1..].parse().map_err(|_| format!("bad id {id}"))?;
            p[i - 1] = Some(
                resp.noul(id)
                    .ok_or_else(|| format!("the response has no noul for `{id}`"))?,
            );
        }
        progress(n + 1, reqs.len());
    }
    let mut out: Vec<Ranked> = candidates
        .iter()
        .zip(p)
        .enumerate()
        .map(|(i, (text, p))| Ranked {
            index: i + 1,
            text: text.clone(),
            p: p.unwrap_or(0.0),
        })
        .collect();
    out.sort_by(|a, b| b.p.total_cmp(&a.p).then(a.index.cmp(&b.index)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_chunked_by_count_and_by_text() {
        let c: Vec<String> = (0..65).map(|i| format!("line {i}")).collect();
        let r = requests("q", &c, INSTRUCTIONS);
        assert_eq!(
            r.iter().map(|x| x.questions.len()).collect::<Vec<_>>(),
            vec![30, 30, 5]
        );
        assert_eq!(r[0].state, json!({"query": "q"}));
        assert!(r[2].questions.contains_key("c65"));
        let q = &r[0].questions["c1"];
        assert_eq!(q["type"], "noul");
        assert!(q["instructions"]
            .as_str()
            .unwrap()
            .ends_with("candidate: line 0"));
        assert_eq!(q["criteria"]["true"], TRUE);
        // Long candidates close a chunk early.
        let long: Vec<String> = (0..3).map(|_| "x".repeat(10_000)).collect();
        let r = requests("q", &long, INSTRUCTIONS);
        assert_eq!(
            r.iter().map(|x| x.questions.len()).collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert!(requests("q", &[], INSTRUCTIONS).is_empty());
    }

    #[test]
    fn every_request_is_one_the_server_accepts() {
        let c: Vec<String> = (0..40).map(|i| format!("candidate {i}")).collect();
        for r in requests("which one", &c, INSTRUCTIONS) {
            crate::protocol::parse_questions(&r.questions).unwrap();
        }
    }
}
