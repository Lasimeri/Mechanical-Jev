//! The client for a System One server: `POST /v1/systemone` and
//! `GET /v1/models`. By default the server is Intel Phi Jev's `xks` on this
//! machine (`http://127.0.0.1:8090`), which answers the same wire format
//! from a local model on this host and its Xeon Phi cards, and needs no
//! key. A key, when set, goes as a bearer token; a 429 or 529 is retried
//! with exponential backoff, honouring `Retry-After`; 401 and 422 are not.
//! See client.md.

use std::time::{Duration, Instant};

use serde_json::Value;
use thiserror::Error;

use crate::protocol::{parse_questions, Request, Response};

/// Intel Phi Jev's server on this machine.
pub const DEFAULT_BASE: &str = "http://127.0.0.1:8090";
pub const DEFAULT_MODEL: &str = "jev-latest";

/// The longest a `Retry-After` is honoured for one wait.
pub const MAX_RETRY_WAIT: Duration = Duration::from_secs(60);

/// A `Retry-After` in seconds, when it is one (finite, not negative), at
/// most [`MAX_RETRY_WAIT`]. The HTTP-date form and anything else read as
/// none, and the client's own backoff applies.
pub fn retry_after(header: Option<&str>) -> Option<Duration> {
    let s: f64 = header?.trim().parse().ok()?;
    (s.is_finite() && s >= 0.0)
        .then(|| Duration::from_secs_f64(s.min(MAX_RETRY_WAIT.as_secs_f64())))
}

/// An environment variable that is set and not blank.
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

#[derive(Debug, Error)]
pub enum JevError {
    #[error("the request is invalid: {0}")]
    Invalid(String),
    #[error("401: the server wants a key (TYPESAFE_API_KEY, e.g. in mjev.local.conf)")]
    Unauthorized,
    #[error("422: the request failed validation: {0}")]
    Unprocessable(String),
    #[error("{0}: {1}")]
    Status(u16, String),
    #[error("gave up after {0} attempts: {1}")]
    Exhausted(u32, String),
    #[error("transport: {0}")]
    Transport(String),
    #[error("malformed response: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone)]
pub struct Client {
    pub base: String,
    /// Sent as a bearer token when set; the local server needs none.
    pub api_key: Option<String>,
    pub model: String,
    /// Attempts for a 429 or 529, the first included.
    pub attempts: u32,
    /// A request's time limit. A local model reads a long session in
    /// seconds to minutes, not the hosted service's milliseconds.
    pub timeout: Duration,
}

impl Client {
    /// From the environment: `TYPESAFE_BASE_URL` (default Intel Phi Jev's
    /// server on this machine), `TYPESAFE_API_KEY` (optional),
    /// `TYPESAFE_DEFAULT_MODEL` (the official SDKs' variable names, so the
    /// same settings drive them).
    pub fn from_env() -> Self {
        Self {
            base: env_nonempty("TYPESAFE_BASE_URL")
                .unwrap_or_else(|| DEFAULT_BASE.into())
                .trim()
                .trim_end_matches('/')
                .to_string(),
            api_key: env_nonempty("TYPESAFE_API_KEY").map(|k| k.trim().to_string()),
            model: env_nonempty("TYPESAFE_DEFAULT_MODEL").unwrap_or_else(|| DEFAULT_MODEL.into()),
            attempts: 5,
            timeout: Duration::from_secs(600),
        }
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new().timeout(self.timeout).build()
    }

    fn authorize(&self, r: ureq::Request) -> ureq::Request {
        match &self.api_key {
            Some(k) => r.set("Authorization", &format!("Bearer {k}")),
            None => r,
        }
    }

    /// Whether the server answers `GET /health`.
    pub fn healthy(&self) -> bool {
        ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(2))
            .build()
            .get(&format!("{}/health", self.base))
            .call()
            .is_ok()
    }

    /// Ask the server. Returns the response and the end-to-end time of the
    /// call that succeeded.
    pub fn system_one(&self, req: &Request) -> Result<(Response, Duration), JevError> {
        parse_questions(&req.questions).map_err(JevError::Invalid)?;
        let mut body = serde_json::to_value(req).map_err(|e| JevError::Invalid(e.to_string()))?;
        if req.model.is_none() {
            body["model"] = Value::from(self.model.clone());
        }
        let url = format!("{}/v1/systemone", self.base);
        let agent = self.agent();
        let mut wait = Duration::from_millis(500);
        let mut last = String::new();
        for attempt in 1..=self.attempts {
            let t0 = Instant::now();
            let r = self.authorize(agent.post(&url)).send_json(body.clone());
            let took = t0.elapsed();
            match r {
                Ok(resp) => {
                    let v: Value = resp
                        .into_json()
                        .map_err(|e| JevError::Malformed(e.to_string()))?;
                    let out: Response = serde_json::from_value(v)
                        .map_err(|e| JevError::Malformed(e.to_string()))?;
                    return Ok((out, took));
                }
                Err(ureq::Error::Status(code, resp)) => {
                    let retry_after = retry_after(resp.header("Retry-After"));
                    let text = resp.into_string().unwrap_or_default();
                    match code {
                        401 => return Err(JevError::Unauthorized),
                        422 => return Err(JevError::Unprocessable(text)),
                        429 | 529 => {
                            last = format!("{code}: {text}");
                            if attempt < self.attempts {
                                std::thread::sleep(retry_after.unwrap_or(wait));
                                wait = (wait * 2).min(Duration::from_secs(30));
                            }
                        }
                        other => return Err(JevError::Status(other, text)),
                    }
                }
                Err(e) => return Err(JevError::Transport(e.to_string())),
            }
        }
        Err(JevError::Exhausted(self.attempts, last))
    }

    /// The models the server serves (`GET /v1/models`).
    pub fn models(&self) -> Result<Value, JevError> {
        let url = format!("{}/v1/models", self.base);
        match self.authorize(self.agent().get(&url)).call() {
            Ok(r) => r
                .into_json()
                .map_err(|e| JevError::Malformed(e.to_string())),
            Err(ureq::Error::Status(401, _)) => Err(JevError::Unauthorized),
            Err(ureq::Error::Status(code, r)) => {
                Err(JevError::Status(code, r.into_string().unwrap_or_default()))
            }
            Err(e) => Err(JevError::Transport(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{retry_after, MAX_RETRY_WAIT};
    use std::time::Duration;

    #[test]
    fn retry_after_is_bounded_and_never_panics() {
        assert_eq!(retry_after(Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(retry_after(Some(" 0.5 ")), Some(Duration::from_millis(500)));
        assert_eq!(retry_after(Some("3600")), Some(MAX_RETRY_WAIT));
        for bad in [
            "-1",
            "inf",
            "NaN",
            "1e300",
            "Wed, 21 Oct 2015 07:28:00 GMT",
            "",
        ] {
            let d = retry_after(Some(bad));
            assert!(d.is_none() || d == Some(MAX_RETRY_WAIT), "{bad}: {d:?}");
        }
        assert_eq!(retry_after(None), None);
    }
}
