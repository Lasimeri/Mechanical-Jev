//! `mjev`: TypeSafe's Jev from the command line. See main.md.

use std::path::PathBuf;
use std::time::Instant;

use std::io::IsTerminal;

use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{json, Map, Value};

use mechanical_jev::client::Client;
use mechanical_jev::policy::{self, Policy};
use mechanical_jev::protocol::Request;
use mechanical_jev::{config, corroborate, eval, fit, guard, label, phi, rank, reconstruction};

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

/// One request: a JSON file (--file), stdin, or --state with
/// --noul/--choice/--score. Shared by `query` and `gate`.
#[derive(clap::Args, Clone)]
struct Ask {
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
}

impl Ask {
    fn request(self) -> Result<Request, String> {
        build_request(self.file, self.state, self.noul, self.choice, self.score)
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// One request: a JSON file (--file), stdin, or --state with
    /// --noul/--choice/--score.
    Query {
        #[command(flatten)]
        ask: Ask,
        /// Print the answers as bars, as the TUI draws them, not JSON.
        #[arg(long)]
        bars: bool,
    },
    /// Ask, then judge the answers: act, review or escalate, the exit code
    /// 0, 10 or 11 (1 is an error, never a decision). Thresholds from
    /// --policy (src/policy.md), else TypeSafe's worked examples: a Noul
    /// acts at 0.9 or 0.1; a Choice or Score escalates under confidence
    /// 0.5 and acts from 0.9.
    Gate {
        #[command(flatten)]
        ask: Ask,
        /// A policy file: thresholds per question, answers to ignore,
        /// weighted composites.
        #[arg(long)]
        policy: Option<PathBuf>,
        /// The verdict as JSON, not lines.
        #[arg(long)]
        json: bool,
    },
    /// The same questions over many states: one JSON line out per line in
    /// (a JSON object with `state` and an optional `id`, any other JSON
    /// value, or plain text), each judged act, review or escalate by a
    /// policy; the count of each on stderr (src/label.md).
    Label {
        /// The states, one per line (default: stdin).
        #[arg(long)]
        input: Option<PathBuf>,
        /// Every line is a text state, even one that parses as JSON.
        #[arg(long)]
        text: bool,
        /// The questions: a JSON file of the questions map, or a request.
        #[arg(long)]
        questions: Option<PathBuf>,
        /// id=instructions (as for `query`).
        #[arg(long)]
        noul: Vec<String>,
        /// id=instructions|options (as for `query`).
        #[arg(long)]
        choice: Vec<String>,
        /// id=instructions|levels (as for `query`).
        #[arg(long)]
        score: Vec<String>,
        /// A policy file (as for `gate`).
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Where the lines go (default: stdout).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Rank candidates against a query, surest first: one Noul per
    /// candidate, TypeSafe's re-ranking shape, its P(yes) the score
    /// (src/rank.md).
    Rank {
        /// What the candidates are ranked against.
        #[arg(long)]
        query: String,
        /// The candidates, one per non-empty line (default: stdin).
        #[arg(long)]
        input: Option<PathBuf>,
        /// Only the best N.
        #[arg(long)]
        top: Option<usize>,
        /// Ask this of each candidate instead of "Does the candidate answer
        /// `query`?".
        #[arg(long)]
        instructions: Option<String>,
        /// The ranking as JSON, not lines.
        #[arg(long)]
        json: bool,
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
    /// Thresholds from recorded answers (`eval --rows`) and what they
    /// should have been: where each question's answers can be trusted,
    /// and a policy that acts only there. Offline: asks nothing
    /// (src/fit.md).
    Fit {
        /// Rows from `mjev eval --rows`.
        rows: PathBuf,
        /// The share of acted-on answers that must be right.
        #[arg(long, default_value_t = 0.95)]
        target: f64,
        /// Write the suggested policy here (for `gate` and `label`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// The report as JSON, not lines.
        #[arg(long)]
        json: bool,
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
    /// A Claude Code PreToolUse hook for Bash: reads the hook's input on
    /// stdin, asks whether the command is risky, and has Claude Code ask
    /// you when it surely is. Never starts a server; on any failure it
    /// prints nothing and Claude Code's permissions run as always. Always
    /// exits 0 (src/guard.md).
    Guard {
        /// Deny a surely risky command (Claude is told why) instead of
        /// asking you.
        #[arg(long)]
        deny: bool,
        /// Allow a surely safe command without the permission prompt.
        #[arg(long)]
        allow_safe: bool,
        /// Other questions: a JSON file of Nouls whose yes means risky.
        #[arg(long)]
        questions: Option<PathBuf>,
        /// A policy file for them (as for `gate`).
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Seconds to wait for the answer before staying out of the way.
        #[arg(long, default_value_t = 20)]
        timeout: u64,
        /// Ask a server that is not on this machine (every command goes
        /// to it); by default the guard steps aside for one.
        #[arg(long)]
        remote: bool,
    },
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
    if let Cmd::Fit {
        rows,
        target,
        out,
        json,
    } = &cmd
    {
        if !(0.5..=1.0).contains(target) {
            return Err(format!("--target is {target}; it must be 0.5 to 1"));
        }
        let r = read_rows(rows)?;
        if r.is_empty() {
            return Err(format!("{}: no rows", rows.display()));
        }
        let fitted = fit::fit(&r, *target);
        if *json {
            println!("{}", serde_json::to_string_pretty(&fitted).unwrap());
        } else {
            print!("{}", fit::text(&fitted, *target));
        }
        if let Some(p) = out {
            let policy = fit::policy(&fitted);
            policy.check()?;
            std::fs::write(p, serde_json::to_string_pretty(&policy).unwrap() + "\n")
                .map_err(|e| format!("{}: {e}", p.display()))?;
            eprintln!("fit: the suggested policy is in {}", p.display());
        }
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
    // Also before phi::ensure: a hook must never start a server.
    if let Cmd::Guard {
        deny,
        allow_safe,
        questions,
        policy,
        timeout,
        remote,
    } = &cmd
    {
        let said = (|| -> Result<Option<Value>, String> {
            let q = match questions {
                Some(p) => label::questions_of(
                    serde_json::from_str(
                        &std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?,
                    )
                    .map_err(|e| format!("{}: {e}", p.display()))?,
                )?,
                None => guard::questions(),
            };
            let p = match policy {
                Some(path) => Policy::load(path)?,
                None => Policy::default(),
            };
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                .map_err(|e| format!("stdin: {e}"))?;
            let c = guard::client(client, std::time::Duration::from_secs(*timeout));
            let mode = guard::Mode {
                deny: *deny,
                allow_safe: *allow_safe,
                remote: *remote,
            };
            guard::run(&c, &input, &q, &p, mode)
        })();
        match said {
            Ok(Some(v)) => println!("{v}"),
            Ok(None) => {}
            // Out of the way: nothing on stdout, the reason on stderr.
            Err(e) => eprintln!("mjev guard: {e}"),
        }
        std::process::exit(0);
    }
    match cmd {
        // Starts nothing up front: the TUI starts the server when asked to.
        Cmd::Tui { file } => return mechanical_jev::tui::app::run(client, file),
        Cmd::Serve => return phi::serve(&client),
        Cmd::Stop => return phi::stop(),
        _ => {}
    }
    // What a command sends is read and checked before the server can be
    // started (phi::ensure: a model load), so a mistake costs a moment.
    let request = match &cmd {
        Cmd::Query { ask, .. } | Cmd::Gate { ask, .. } => Some(ask.clone().request()?),
        _ => None,
    };
    let gate_policy = match (&cmd, &request) {
        (Cmd::Gate { policy, .. }, Some(req)) => {
            let p = match policy {
                Some(path) => Policy::load(path)?,
                None => Policy::default(),
            };
            p.check_questions(&policy::asked(req)?)?;
            Some(p)
        }
        _ => None,
    };
    // label: its states, questions and policy; rank: its candidates.
    let labelling = match &cmd {
        Cmd::Label {
            input,
            text,
            questions,
            noul,
            choice,
            score,
            policy,
            ..
        } => {
            let items = label::items(&read_input(input.as_ref())?, *text);
            if items.is_empty() {
                return Err("no states: every line of the input is empty".into());
            }
            let mut q = match questions {
                Some(p) => label::questions_of(
                    serde_json::from_str(
                        &std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?,
                    )
                    .map_err(|e| format!("{}: {e}", p.display()))?,
                )?,
                None => Map::new(),
            };
            if !(noul.is_empty() && choice.is_empty() && score.is_empty()) {
                let flags = build_request(
                    None,
                    Some(String::new()),
                    noul.clone(),
                    choice.clone(),
                    score.clone(),
                )?;
                q.extend(flags.questions);
            }
            if q.is_empty() {
                return Err("no questions: --questions FILE, or --noul/--choice/--score".into());
            }
            mechanical_jev::protocol::parse_questions(&q)?;
            let p = match policy {
                Some(path) => Policy::load(path)?,
                None => Policy::default(),
            };
            Some((items, q, p))
        }
        _ => None,
    };
    let candidates = match &cmd {
        Cmd::Rank { input, .. } => {
            let c: Vec<String> = read_input(input.as_ref())?
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            if c.is_empty() {
                return Err("no candidates: every line of the input is empty".into());
            }
            Some(c)
        }
        _ => None,
    };
    phi::ensure(&client)?;
    match cmd {
        Cmd::Label { out, .. } => {
            let (items, q, p) = labelling.ok_or("no states were read")?;
            let mut sink: Box<dyn std::io::Write> = match &out {
                Some(path) => Box::new(
                    std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?,
                ),
                None => Box::new(std::io::stdout().lock()),
            };
            let t0 = Instant::now();
            let tty = std::io::stderr().is_terminal();
            let tally = label::run(&client, &q, &p, &items, &mut sink, &mut |d, n| {
                if tty {
                    eprint!("\rlabel: {d} of {n}");
                }
            })?;
            if tty {
                eprintln!();
            }
            eprintln!(
                "label: {} states in {:.1} s: act {}, review {}, escalate {}, errors {}",
                items.len(),
                t0.elapsed().as_secs_f64(),
                tally.act,
                tally.review,
                tally.escalate,
                tally.errors
            );
            if tally.errors > 0 {
                return Err(format!(
                    "{} of {} states failed (their lines say why)",
                    tally.errors,
                    items.len()
                ));
            }
            Ok(())
        }
        Cmd::Rank {
            query,
            top,
            instructions,
            json,
            ..
        } => {
            let c = candidates.ok_or("no candidates were read")?;
            let t0 = Instant::now();
            let words = instructions.unwrap_or_else(|| rank::INSTRUCTIONS.to_string());
            let mut ranked = rank::rank(&client, &query, &c, &words, &mut |_, _| {})?;
            ranked.truncate(top.unwrap_or(ranked.len()));
            if json {
                println!("{}", serde_json::to_string_pretty(&ranked).unwrap());
            } else {
                for r in &ranked {
                    println!("{:.2}  {:>4}  {}", r.p, r.index, r.text);
                }
            }
            eprintln!(
                "rank: {} candidates in {:.1} s",
                c.len(),
                t0.elapsed().as_secs_f64()
            );
            Ok(())
        }
        Cmd::Gate { json, .. } => {
            let (req, p) = request.zip(gate_policy).ok_or("no request was read")?;
            let (resp, took) = client.system_one(&req).map_err(|e| e.to_string())?;
            let v = p.judge(&policy::asked(&req)?, &resp)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                print!("{}", policy::text(&v));
            }
            eprintln!("{}: {:.1} ms", resp.model, took.as_secs_f64() * 1e3);
            std::process::exit(v.outcome.exit_code());
        }
        Cmd::Query { bars, .. } => {
            let req = request.ok_or("no request was read")?;
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
        | Cmd::Fit { .. }
        | Cmd::Guard { .. }
        | Cmd::Reconstruct { .. }
        | Cmd::Evidence { .. }
        | Cmd::Serve
        | Cmd::Stop
        | Cmd::Doctor { .. }
        | Cmd::Tui { .. } => Ok(()),
    }
}

/// A file's text, or stdin's; refused at once when stdin is a terminal
/// and nothing names a file, rather than waiting on it.
fn read_input(file: Option<&PathBuf>) -> Result<String, String> {
    match file {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display())),
        None if std::io::stdin().is_terminal() => {
            Err("no input: --input FILE, or lines piped on stdin".into())
        }
        None => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|e| format!("stdin: {e}"))?;
            Ok(text)
        }
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
        // At a terminal with nothing piped, say what is missing rather
        // than wait on stdin.
        if std::io::stdin().is_terminal() {
            return Err(
                "no request: --file req.json, --state with --noul/--choice/--score, \
                        or a JSON request piped on stdin"
                    .into(),
            );
        }
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|e| format!("stdin: {e}"))?;
        return serde_json::from_str(&text).map_err(|e| format!("the request on stdin: {e}"));
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
