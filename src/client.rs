//! The client for TypeSafe's API: `POST /v1/systemone` and `GET /v1/models`,
//! with the key as a bearer token and the retries TypeSafe documents:
//! a 429 (rate limit) or 529 (overloaded) is retried with exponential
//! backoff, honouring `Retry-After`; 401 and 422 are not. See client.md.

use std::time::{Duration, Instant};

use serde_json::Value;
use thiserror::Error;

use crate::protocol::{parse_questions, Request, Response};

pub const DEFAULT_BASE: &str = "https://api.typesafe.ai";
pub const DEFAULT_MODEL: &str = "jev-latest";

#[derive(Debug, Error)]
pub enum JevError {
    #[error("no API key: set TYPESAFE_API_KEY (console.typesafe.ai), e.g. in mjev.local.conf")]
    NoKey,
    #[error("the request is invalid: {0}")]
    Invalid(String),
    #[error("401: missing or invalid API key")]
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
    pub api_key: String,
    pub model: String,
    /// Attempts for a 429 or 529, the first included.
    pub attempts: u32,
    pub timeout: Duration,
}

impl Client {
    /// From the environment: `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`,
    /// `TYPESAFE_DEFAULT_MODEL` (the official SDKs' variable names).
    pub fn from_env() -> Result<Self, JevError> {
        let api_key = std::env::var("TYPESAFE_API_KEY").map_err(|_| JevError::NoKey)?;
        if api_key.trim().is_empty() {
            return Err(JevError::NoKey);
        }
        Ok(Self {
            base: std::env::var("TYPESAFE_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE.into())
                .trim_end_matches('/')
                .to_string(),
            api_key,
            model: std::env::var("TYPESAFE_DEFAULT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
            attempts: 5,
            timeout: Duration::from_secs(60),
        })
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new().timeout(self.timeout).build()
    }

    /// Ask Jev. Returns the response and the end-to-end time of the call
    /// that succeeded.
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
            let r = agent
                .post(&url)
                .set("Authorization", &format!("Bearer {}", self.api_key))
                .send_json(body.clone());
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
                    let retry_after = resp
                        .header("Retry-After")
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .map(Duration::from_secs_f64);
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

    /// The models this key can use (`GET /v1/models`), as TypeSafe returns them.
    pub fn models(&self) -> Result<Value, JevError> {
        let url = format!("{}/v1/models", self.base);
        match self
            .agent()
            .get(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .call()
        {
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
