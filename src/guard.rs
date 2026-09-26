//! A guardrail for an agent's shell commands: `mjev guard`, a Claude Code
//! PreToolUse hook that asks Jev whether a command is risky before it
//! runs, and has Claude Code ask the person when it surely is. It never
//! starts a server and never blocks on a failure: when anything goes
//! wrong it says nothing, and Claude Code's own permission flow runs as
//! it would have. See guard.md.

use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::policy::{self, Outcome, Policy, Verdict};
use crate::protocol::Request;

/// What matters of a PreToolUse hook's input (Claude Code's hooks guide:
/// `tool_name`, `tool_input`, `cwd` among others; a Bash call's
/// `tool_input` holds its `command`).
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub tool: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

pub fn call(input: &Value) -> Call {
    let s = |v: &Value| v.as_str().map(String::from);
    Call {
        tool: s(&input["tool_name"]).unwrap_or_default(),
        command: s(&input["tool_input"]["command"]),
        cwd: s(&input["cwd"]),
    }
}

/// How the guard answers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Mode {
    /// A surely risky command is denied (Claude is told why) instead of
    /// put to the person.
    pub deny: bool,
    /// A surely safe command is allowed without the prompt. Off by
    /// default: it hands a permission to the model's reading.
    pub allow_safe: bool,
}

/// The hook's stdout for a risk, or nothing (Claude Code's own
/// permission flow then runs unchanged). `ask` puts the call to the
/// person as a permission prompt; `deny` cancels it and tells Claude.
pub fn output(risk: &Risk, mode: Mode) -> Option<Value> {
    let (decision, reason) = match risk {
        Risk::Risky(why) if mode.deny => (
            "deny",
            format!("Intel Phi Jev reads this command as risky ({why}); mjev guard denied it"),
        ),
        Risk::Risky(why) => (
            "ask",
            format!("Intel Phi Jev reads this command as risky ({why}); mjev guard asks you first"),
        ),
        Risk::Safe if mode.allow_safe => (
            "allow",
            "Intel Phi Jev reads this command as surely safe (mjev guard --allow-safe)".into(),
        ),
        _ => return None,
    };
    Some(json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": decision,
        "permissionDecisionReason": reason,
    }}))
}

/// The whole hook: the input read, a Bash command asked about, the
/// answers judged. `Err` for anything that went wrong (the caller then
/// prints nothing); `Ok(None)` for nothing to say. It never starts a
/// server: one that does not answer its health check in two seconds is
/// a reason to stay out of the way, not to load a model in a tool call.
pub fn run(
    client: &Client,
    input: &str,
    questions: &Map<String, Value>,
    policy: &Policy,
    mode: Mode,
) -> Result<Option<Value>, String> {
    let c = call(&serde_json::from_str(input).map_err(|e| format!("hook input: {e}"))?);
    let Some(command) = c.command.filter(|_| c.tool == "Bash") else {
        return Ok(None);
    };
    client
        .health()
        .map_err(|e| format!("no server answers: {e}"))?;
    let req = Request {
        model: None,
        state: json!({"command": command, "cwd": c.cwd.unwrap_or_default()}),
        questions: questions.clone(),
    };
    let asked = policy::asked(&req)?;
    let (resp, _) = client.system_one(&req).map_err(|e| e.to_string())?;
    let v = policy.judge(&asked, &resp)?;
    Ok(output(&risk(&v), mode))
}

/// A client for the hook: one attempt, and no longer than `budget` (well
/// inside the hook's own timeout in Claude Code's settings).
pub fn client(mut c: Client, budget: Duration) -> Client {
    c.attempts = 1;
    c.timeout = budget;
    c
}

/// The questions asked of a command unless the caller gives their own:
/// each a Noul whose yes means risky, worded as the jev-1.13 jaggedness
/// notes advise (the exact condition, the state's field by name, criteria
/// for each side).
pub fn questions() -> Map<String, Value> {
    let q = |instructions: &str, yes: &str, no: &str| {
        json!({"type": "noul", "instructions": instructions,
               "criteria": {"true": yes, "false": no}})
    };
    let mut m = Map::new();
    m.insert(
        "destroys".into(),
        q(
            "Would running `command` delete, overwrite or reset files, data or history in a way that cannot be undone?",
            "It removes or overwrites something with no copy left: rm -rf, a forced reset or push, dropping a table, truncating a file.",
            "It only reads, builds, tests, or makes changes that can be undone.",
        ),
    );
    m.insert(
        "outside".into(),
        q(
            "Does `command` change anything outside `cwd`, the project directory?",
            "It writes to the home directory, system paths, other projects, or remote machines.",
            "It only reads, or writes inside `cwd`.",
        ),
    );
    m
}

/// What the answers say about a command.
#[derive(Debug, Clone, PartialEq)]
pub enum Risk {
    /// A question surely said yes: why, for the person to read.
    Risky(String),
    /// Every question surely said no.
    Safe,
    /// Not sure either way.
    Unsure,
}

/// A verdict on a command's questions: risky when any deciding question
/// surely said yes, safe when every one surely said no, else unsure.
pub fn risk(v: &Verdict) -> Risk {
    let deciding: Vec<_> = v.answers.iter().filter(|j| !j.ignored).collect();
    let yes: Vec<String> = deciding
        .iter()
        .filter(|j| j.decision == "yes" && j.outcome == Outcome::Act)
        .map(|j| format!("{} p(yes) {:.2}", j.id, j.value.as_f64().unwrap_or(0.0)))
        .collect();
    if !yes.is_empty() {
        return Risk::Risky(yes.join(", "));
    }
    let all_no = !deciding.is_empty()
        && deciding
            .iter()
            .all(|j| j.decision == "no" && j.outcome == Outcome::Act);
    if all_no {
        Risk::Safe
    } else {
        Risk::Unsure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Policy;
    use crate::protocol::Response;

    fn verdict(destroys: f64, outside: f64) -> Verdict {
        let resp: Response = serde_json::from_value(json!({"model": "m", "answers": {
            "destroys": {"type": "noul", "noul": destroys},
            "outside": {"type": "noul", "noul": outside}}}))
        .unwrap();
        let asked = vec![
            ("destroys".to_string(), "noul".to_string()),
            ("outside".to_string(), "noul".to_string()),
        ];
        Policy::default().judge(&asked, &resp).unwrap()
    }

    #[test]
    fn a_sure_yes_is_risky_every_sure_no_is_safe_the_rest_unsure() {
        assert_eq!(
            risk(&verdict(0.97, 0.02)),
            Risk::Risky("destroys p(yes) 0.97".into())
        );
        assert_eq!(risk(&verdict(0.03, 0.05)), Risk::Safe);
        assert_eq!(risk(&verdict(0.03, 0.5)), Risk::Unsure);
        assert_eq!(risk(&verdict(0.6, 0.6)), Risk::Unsure);
    }

    #[test]
    fn the_questions_are_ones_the_server_accepts() {
        crate::protocol::parse_questions(&questions()).unwrap();
    }

    #[test]
    fn a_hook_input_gives_its_tool_command_and_directory() {
        let input = json!({"session_id": "s", "hook_event_name": "PreToolUse",
            "cwd": "/home/user/my-project", "tool_name": "Bash",
            "tool_input": {"command": "npm test"}});
        assert_eq!(
            call(&input),
            Call {
                tool: "Bash".into(),
                command: Some("npm test".into()),
                cwd: Some("/home/user/my-project".into())
            }
        );
        assert_eq!(call(&json!({})).command, None);
    }

    #[test]
    fn only_a_sure_risk_speaks_unless_asked_to_allow_safe_ones() {
        let risky = Risk::Risky("destroys p(yes) 0.97".into());
        let o = output(&risky, Mode::default()).unwrap();
        assert_eq!(o["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(o["hookSpecificOutput"]["permissionDecision"], "ask");
        assert!(o["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("0.97"));
        let deny = Mode {
            deny: true,
            ..Mode::default()
        };
        assert_eq!(
            output(&risky, deny).unwrap()["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        assert_eq!(output(&Risk::Safe, Mode::default()), None);
        assert_eq!(output(&Risk::Unsure, Mode::default()), None);
        let allow = Mode {
            allow_safe: true,
            ..Mode::default()
        };
        assert_eq!(
            output(&Risk::Safe, allow).unwrap()["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert_eq!(output(&Risk::Unsure, allow), None);
    }

    #[test]
    fn a_server_that_does_not_answer_is_an_error_and_other_tools_are_nothing() {
        let c = client(
            Client {
                base: "http://127.0.0.1:9".into(),
                api_key: None,
                model: "jev-latest".into(),
                attempts: 3,
                timeout: Duration::from_secs(60),
            },
            Duration::from_secs(1),
        );
        assert_eq!(c.attempts, 1);
        let q = questions();
        let p = Policy::default();
        let bash = r#"{"tool_name": "Bash", "tool_input": {"command": "ls"}, "cwd": "/"}"#;
        assert!(run(&c, bash, &q, &p, Mode::default()).is_err());
        let edit = r#"{"tool_name": "Edit", "tool_input": {"file_path": "/x"}}"#;
        assert_eq!(run(&c, edit, &q, &p, Mode::default()), Ok(None));
        assert!(run(&c, "not json", &q, &p, Mode::default()).is_err());
    }
}
