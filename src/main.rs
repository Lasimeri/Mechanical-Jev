//! `mjev`: TypeSafe's Jev from the command line. See main.md.

use std::path::PathBuf;
use std::time::Instant;

use std::io::IsTerminal;

use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{json, Map, Value};

use mechanical_jev::client::Client;
use mechanical_jev::protocol::Request;
use mechanical_jev::{config, corroborate, eval, phi, reconstruction};

#[derive(Parser)]
#[command(
    name = "mjev",
    version,
    about = "Mechanical Jev: System One questions (noul, choice, score) to Intel Phi Jev from the command line; `mjev` alone, in a terminal, opens the TUI"
)]
struct Cli {
    #[command(subcommand)]
    /// None: the TUI in a terminal (`mjev` alone).
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// One request: a JSON file (--file), stdin, or --state with
    /// --noul/--choice/--score.
    Query {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        state: Option<String>,
        /// id=instructions
        #[arg(long)]
        noul: Vec<String>,
        /// id=instructions|key1:desc,key2:desc (`;` between options when a
        /// description has commas; the last `|` starts the options)
        #[arg(long)]
        choice: Vec<String>,
        /// id=instructions|level0,level1,level2 (`;` between levels when one
        /// has commas)
        #[arg(long)]
        score: Vec<String>,
        /// Print the answers as bars, as the TUI draws them, not JSON.
        #[arg(long)]
        bars: bool,
    },
    /// Score a labelled JSONL case file with Jev: accuracy, Brier, ECE,
    /// coverage, latency.
    Eval {
        file: PathBuf,
        /// Record every answer here (for `corroborate`).
        #[arg(long)]
        rows: Option<PathBuf>,
        /// Only the first N cases.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Compare two recorded runs question by question.
    Corroborate {
        a: PathBuf,
        b: PathBuf,
        /// Also list every pair.
        #[arg(long)]
        pairs: bool,
        /// Also find, per question kind, the temperature that brings B closest to A.
        #[arg(long)]
        temperature: bool,
    },
    /// The models the server serves.
    Models,
    /// Jev's published answers as a case file (gold = Jev's answer) and as
    /// eval rows, to measure how closely a server answers like Jev.
    Evidence {
        /// Directory to write evidence-cases.jsonl and evidence-jev-rows.jsonl.
        #[arg(long, default_value = "target")]
        out: PathBuf,
    },
    /// What Jev most likely does with a request, reconstructed from its
    /// documentation (docs/reverse-engineering.md): the document and each
    /// question's branch as the model reads it. Offline; asks nothing.
    Reconstruct {
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Start Intel Phi Jev's server (the other commands start it when needed).
    Serve,
    /// Stop Intel Phi Jev's server and release the Phi cards.
    Stop,
    /// The family's setup check: mjev, Intel Phi Jev's xks (and its own
    /// `xks doctor`, down to the cards), the server; what is missing and
    /// the fix for each. Starts nothing. Exit 0 ready, 1 not.
    Doctor {
        /// Do the fixes that are a build or a link: xks built, mjev and xks
        /// linked into PREFIX/bin, the payload built. Never a card, a
        /// download or sudo.
        #[arg(long)]
        fix: bool,
        /// Where --fix links the commands (PREFIX/bin).
        #[arg(long, default_value = "~/.local")]
        prefix: String,
    },
    /// The terminal interface: write a state and questions, ask, read the
    /// answers as bars; the server started and stopped from the same screen.
    Tui {
        /// A request file to open with.
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

fn main() {
    config::load();
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn read_rows(p: &PathBuf) -> Result<Vec<eval::Row>, String> {
    std::fs::read_to_string(p)
        .map_err(|e| format!("{}: {e}", p.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", p.display())))
        .collect()
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let cmd = match cli.cmd {
        Some(c) => c,
        // `mjev` alone: the TUI in a terminal; anywhere else the usage
        // error it always was (exit 2).
        None if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() => {
            Cmd::Tui { file: None }
        }
        None => Cli::command()
            .error(
                clap::error::ErrorKind::MissingSubcommand,
                "a subcommand is required (mjev alone opens the TUI, in a terminal)",
            )
            .exit(),
    };
    if let Cmd::Evidence { out } = &cmd {
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let cases: String = mechanical_jev::evidence::cases()
            .iter()
            .map(|c| serde_json::to_string(c).unwrap() + "\n")
            .collect();
        let rows: String = mechanical_jev::evidence::jev_rows()
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect();
        let (cf, rf) = (
            out.join("evidence-cases.jsonl"),
            out.join("evidence-jev-rows.jsonl"),
        );
        std::fs::write(&cf, cases).map_err(|e| e.to_string())?;
        std::fs::write(&rf, rows).map_err(|e| e.to_string())?;
        println!("{}\n{}", cf.display(), rf.display());
        return Ok(());
    }
    if let Cmd::Reconstruct { file } = &cmd {
        let req = build_request(file.clone(), None, Vec::new(), Vec::new(), Vec::new())?;
        let plan = reconstruction::compile(&req)?;
        let branches: Vec<Value> = plan
            .branches
            .iter()
            .map(|b| json!({"id": b.id, "kind": format!("{:?}", b.kind), "reads": b.prompt, "outcomes": b.outcomes}))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "preamble_tokens": reconstruction::PREAMBLE_TOKENS,
                "document": plan.document,
                "branches": branches,
                "limits": {"state_plus_longest_question": reconstruction::BRANCH_LIMIT, "request": reconstruction::REQUEST_LIMIT},
            }))
            .unwrap()
        );
        return Ok(());
    }
    if let Cmd::Corroborate {
        a,
        b,
        pairs,
        temperature,
    } = &cmd
    {
        let (ra, rb) = (read_rows(a)?, read_rows(b)?);
        let mut r = corroborate::compare(&ra, &rb);
        if !pairs {
            r.pairs.clear();
        }
        let mut v = serde_json::to_value(&r).unwrap();
        if *temperature {
            v["temperature"] =
                serde_json::to_value(corroborate::fit_temperature(&ra, &rb)).unwrap();
        }
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
        return Ok(());
    }
    let client = Client::from_env();
    // Before anything that could start the server (phi::ensure below).
    if let Cmd::Doctor { fix, prefix } = &cmd {
        let prefix = match prefix.strip_prefix("~/") {
            Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
            None => PathBuf::from(prefix),
        };
        println!(
            "mjev doctor: Mechanical Jev at {}",
            config::repo_root()
                .map_or_else(|| "(not in a checkout)".into(), |p| p.display().to_string())
        );
        let (text, ready) = mechanical_jev::doctor::run(&client, *fix, &prefix);
        print!("{text}");
        std::process::exit(if ready { 0 } else { 1 });
    }
    match cmd {
        // Starts nothing up front: the TUI starts the server when asked to.
        Cmd::Tui { file } => return mechanical_jev::tui::app::run(client, file),
        Cmd::Serve => return phi::serve(&client),
        Cmd::Stop => return phi::stop(),
        _ => phi::ensure(&client)?,
    }
    match cmd {
        Cmd::Query {
            file,
            state,
            noul,
            choice,
            score,
            bars,
        } => {
            let req = build_request(file, state, noul, choice, score)?;
            let (resp, took) = client.system_one(&req).map_err(|e| e.to_string())?;
            if bars {
                let text = mechanical_jev::tui::model::answers_text(&req, &resp.answers, 30);
                print!("{text}");
            } else {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            }
            eprintln!("{}: {:.1} ms", resp.model, took.as_secs_f64() * 1e3);
            Ok(())
        }
        Cmd::Eval { file, rows, limit } => {
            let mut cases = eval::load_cases(&file)?;
            if let Some(n) = limit {
                cases.truncate(n);
            }
            let t0 = Instant::now();
            let (r, failed) = eval::run(&client, &cases).map_err(|e| e.to_string())?;
            let wall = t0.elapsed().as_secs_f64();
            let mut v = serde_json::to_value(eval::metrics(&r, failed)).unwrap();
            v["model"] = json!(client.model);
            v["cases"] = json!(cases.len());
            v["wall_s"] = json!((wall * 100.0).round() / 100.0);
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            if let Some(p) = rows {
                let text: String = r
                    .iter()
                    .map(|x| serde_json::to_string(x).unwrap() + "\n")
                    .collect();
                std::fs::write(&p, text).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Cmd::Models => {
            let m = client.models().map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&m).unwrap());
            Ok(())
        }
        Cmd::Corroborate { .. }
        | Cmd::Reconstruct { .. }
        | Cmd::Evidence { .. }
        | Cmd::Serve
        | Cmd::Stop
        | Cmd::Doctor { .. }
        | Cmd::Tui { .. } => Ok(()),
    }
}

fn build_request(
    file: Option<PathBuf>,
    state: Option<String>,
    noul: Vec<String>,
    choice: Vec<String>,
    score: Vec<String>,
) -> Result<Request, String> {
    if let Some(p) = file {
        let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
        return serde_json::from_str(&text).map_err(|e| e.to_string());
    }
    if state.is_none() && noul.is_empty() && choice.is_empty() && score.is_empty() {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|e| e.to_string())?;
        return serde_json::from_str(&text).map_err(|e| e.to_string());
    }
    let state = state.ok_or("--state is required with --noul/--choice/--score")?;
    let mut questions = Map::new();
    for s in noul {
        let (id, instr) = split_once(&s, '=')?;
        questions.insert(id.into(), json!({"type": "noul", "instructions": instr}));
    }
    for s in choice {
        let (id, rest) = split_once(&s, '=')?;
        let (instr, opts) = split_last(rest, '|')?;
        let mut criteria = Map::new();
        for o in items(opts, &s)? {
            let (k, d) = o.split_once(':').unwrap_or((o, ""));
            let (k, d) = (k.trim(), d.trim());
            if k.is_empty() {
                return Err(format!("an option without a key in `{s}`"));
            }
            if criteria.contains_key(k) {
                return Err(format!("option `{k}` twice in `{s}`"));
            }
            criteria.insert(k.into(), if d.is_empty() { Value::Null } else { json!(d) });
        }
        questions.insert(
            id.into(),
            json!({"type": "choice", "instructions": instr, "criteria": criteria}),
        );
    }
    for s in score {
        let (id, rest) = split_once(&s, '=')?;
        let (instr, levels) = split_last(rest, '|')?;
        let levels = items(levels, &s)?;
        questions.insert(
            id.into(),
            json!({"type": "score", "instructions": instr, "criteria": levels}),
        );
    }
    Ok(Request {
        model: None,
        state: Value::String(state),
        questions,
    })
}

fn split_once(s: &str, c: char) -> Result<(&str, &str), String> {
    s.split_once(c)
        .map(|(a, b)| (a.trim(), b.trim()))
        .ok_or_else(|| format!("expected `{c}` in `{s}`"))
}

/// Split at the last `c`: the instructions before it may contain `c`.
fn split_last(s: &str, c: char) -> Result<(&str, &str), String> {
    s.rsplit_once(c)
        .map(|(a, b)| (a.trim(), b.trim()))
        .ok_or_else(|| format!("expected `{c}` in `{s}`"))
}

/// A list of options or levels: `;`-separated when there is a `;` (so a
/// description may contain commas), else `,`-separated. Every item
/// non-empty.
fn items<'a>(list: &'a str, whole: &str) -> Result<Vec<&'a str>, String> {
    let sep = if list.contains(';') { ';' } else { ',' };
    let v: Vec<&str> = list.split(sep).map(str::trim).collect();
    if v.iter().any(|x| x.is_empty()) {
        return Err(format!("an empty option or level in `{whole}`"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    #[test]
    fn option_lists_split_as_documented() {
        assert_eq!(super::items("a,b , c", "").unwrap(), ["a", "b", "c"]);
        assert_eq!(
            super::items("billing:Payments, invoicing; technical:Bugs", "").unwrap(),
            ["billing:Payments, invoicing", "technical:Bugs"]
        );
        assert!(super::items("a,,b", "").is_err());
        assert!(super::items("a,", "").is_err());
        assert_eq!(
            super::split_last("Is a|b true?|x,y", '|').unwrap(),
            ("Is a|b true?", "x,y")
        );
    }
}
