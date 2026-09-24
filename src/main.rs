//! `mjev`: TypeSafe's Jev from the command line. See main.md.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};

use mechanical_jev::client::Client;
use mechanical_jev::protocol::Request;
use mechanical_jev::{config, corroborate, eval};

#[derive(Parser)]
#[command(
    name = "mjev",
    version,
    about = "Mechanical Jev: TypeSafe's Jev (System One: noul, choice, score) from the command line"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
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
        /// id=instructions|key1:desc,key2:desc
        #[arg(long)]
        choice: Vec<String>,
        /// id=instructions|level0,level1,level2
        #[arg(long)]
        score: Vec<String>,
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
    },
    /// The models this key can use.
    Models,
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
    if let Cmd::Corroborate { a, b, pairs } = &cli.cmd {
        let mut r = corroborate::compare(&read_rows(a)?, &read_rows(b)?);
        if !pairs {
            r.pairs.clear();
        }
        println!("{}", serde_json::to_string_pretty(&r).unwrap());
        return Ok(());
    }
    let client = Client::from_env().map_err(|e| e.to_string())?;
    match cli.cmd {
        Cmd::Query {
            file,
            state,
            noul,
            choice,
            score,
        } => {
            let req = build_request(file, state, noul, choice, score)?;
            let (resp, took) = client.system_one(&req).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
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
        Cmd::Corroborate { .. } => Ok(()),
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
        let (instr, opts) = split_once(rest, '|')?;
        let mut criteria = Map::new();
        for o in opts.split(',') {
            let (k, d) = o.split_once(':').unwrap_or((o, ""));
            let d = d.trim();
            criteria.insert(
                k.trim().into(),
                if d.is_empty() { Value::Null } else { json!(d) },
            );
        }
        questions.insert(
            id.into(),
            json!({"type": "choice", "instructions": instr, "criteria": criteria}),
        );
    }
    for s in score {
        let (id, rest) = split_once(&s, '=')?;
        let (instr, levels) = split_once(rest, '|')?;
        let levels: Vec<&str> = levels.split(',').map(str::trim).collect();
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
