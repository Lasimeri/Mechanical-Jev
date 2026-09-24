//! jevre: reverse engineering Jev from its published documentation only.
//! Offline: it reads `evidence/published_pairs.json` (the 13 request and
//! response pairs TypeSafe publishes with token counts) and a directory of
//! tokenizer.json files (`JEVRE_TOKENIZERS`, one subdirectory per
//! tokenizer). It calls nothing.
//!
//!   jevre input    the hidden prompt's constants, per tokenizer
//!   jevre output   what output_tokens could count
//!   jevre probs    TypeSafe's confidence formulas, and the sampling test
//!
//! See main.md and ../../docs/reverse-engineering.md.

use std::path::PathBuf;

use serde_json::Value;
use tokenizers::Tokenizer;

struct Pair {
    req: Value,
    resp: Value,
    input: f64,
    output: f64,
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pairs() -> Vec<Pair> {
    let text =
        std::fs::read_to_string(repo().join("evidence/published_pairs.json")).expect("evidence");
    let v: Vec<Value> = serde_json::from_str(&text).expect("evidence json");
    v.into_iter()
        .map(|p| Pair {
            input: p["response"]["usage"]["input_tokens"].as_f64().unwrap(),
            output: p["response"]["usage"]["output_tokens"].as_f64().unwrap(),
            req: p["request"].clone(),
            resp: p["response"].clone(),
        })
        .collect()
}

fn tokenizers() -> Vec<(String, Tokenizer)> {
    let dir = std::env::var("JEVRE_TOKENIZERS")
        .unwrap_or_else(|_| repo().join("tools/jevre/tokenizers").display().to_string());
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e} (see tools/jevre/main.md)"))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for n in names {
        let f = format!("{dir}/{n}/tokenizer.json");
        if let Ok(t) = Tokenizer::from_file(&f) {
            out.push((n, t));
        }
    }
    out
}

fn count(t: &Tokenizer, s: &str) -> f64 {
    if s.is_empty() {
        0.0
    } else {
        t.encode(s, false).map(|e| e.len() as f64).unwrap_or(0.0)
    }
}

/// Least squares, normal equations with Gauss-Jordan (index loops: the
/// elimination reads and writes several rows at once).
#[allow(clippy::needless_range_loop)]
fn lstsq(x: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    let k = x[0].len();
    let mut a = vec![vec![0.0; k + 1]; k];
    for (row, &yy) in x.iter().zip(y) {
        for i in 0..k {
            for j in 0..k {
                a[i][j] += row[i] * row[j];
            }
            a[i][k] += row[i] * yy;
        }
    }
    for c in 0..k {
        let p = (c..k)
            .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
            .unwrap();
        a.swap(c, p);
        let d = a[c][c];
        if d.abs() < 1e-12 {
            continue;
        }
        for j in c..=k {
            a[c][j] /= d;
        }
        for i in 0..k {
            if i != c {
                let f = a[i][c];
                for j in c..=k {
                    a[i][j] -= f * a[c][j];
                }
            }
        }
    }
    (0..k).map(|i| a[i][k]).collect()
}

fn fit(x: &[Vec<f64>], y: &[f64]) -> (Vec<f64>, Vec<f64>, f64) {
    let b = lstsq(x, y);
    let res: Vec<f64> = x
        .iter()
        .zip(y)
        .map(|(r, yy)| yy - r.iter().zip(&b).map(|(a, c)| a * c).sum::<f64>())
        .collect();
    let rms = (res.iter().map(|r| r * r).sum::<f64>() / res.len() as f64).sqrt();
    (b, res, rms)
}

fn state_text(v: &Value, pretty: bool) -> String {
    match v {
        Value::String(s) => s.clone(),
        o if pretty => serde_json::to_string_pretty(o).unwrap(),
        o => serde_json::to_string(o).unwrap(),
    }
}

/// Input: the questions as their JSON objects (no ids), compact or pretty;
/// the state verbatim or as JSON; unknowns: the preamble and one wrapper
/// cost per question type.
fn input() {
    let ps = pairs();
    let mut rows = Vec::new();
    for (name, t) in tokenizers() {
        for qmode in ["compact", "pretty"] {
            for pretty_state in [true, false] {
                let mut x = Vec::new();
                let mut y = Vec::new();
                for p in &ps {
                    let mut text = count(&t, &state_text(&p.req["state"], pretty_state));
                    let mut row = vec![1.0, 0.0, 0.0, 0.0];
                    for q in p.req["questions"].as_object().unwrap().values() {
                        let body = if qmode == "compact" {
                            serde_json::to_string(q).unwrap()
                        } else {
                            serde_json::to_string_pretty(q).unwrap()
                        };
                        text += count(&t, &body);
                        match q["type"].as_str().unwrap() {
                            "noul" => row[1] += 1.0,
                            "choice" => row[2] += 1.0,
                            _ => row[3] += 1.0,
                        }
                    }
                    x.push(row);
                    y.push(p.input - text);
                }
                let (b, res, rms) = fit(&x, &y);
                rows.push((rms, name.clone(), qmode, pretty_state, b, res));
            }
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("input_tokens = preamble + tokens(state) + sum over questions of (tokens(question JSON) + wrapper by type)");
    println!("{} published pairs; best fits first\n", ps.len());
    for (rms, name, q, s, b, res) in rows.iter().take(12) {
        println!(
            "{name:<10} questions {q:<7} state {:<7} rms {rms:5.2}  preamble {:6.1}  wrapper noul {:5.1} choice {:5.1} score {:5.1}  residuals {}",
            if *s { "pretty" } else { "compact" },
            b[0],
            b[1],
            b[2],
            b[3],
            res.iter().map(|r| format!("{r:+.0}")).collect::<Vec<_>>().join(" ")
        );
    }
}

/// Output: is output_tokens a function of the question structure?
fn output() {
    let ps = pairs();
    println!("output_tokens as published, with each response's structure:");
    for p in &ps {
        let s: Vec<String> = p.req["questions"]
            .as_object()
            .unwrap()
            .values()
            .map(|q| match q["type"].as_str().unwrap() {
                "noul" => "noul".to_string(),
                "choice" => format!("choice{}", q["criteria"].as_object().unwrap().len()),
                _ => format!("score{}", q["criteria"].as_array().unwrap().len()),
            })
            .collect();
        println!("  {:>4}  {}", p.output, s.join(" + "));
    }
    // an additive model: constant + per type + per option
    let mut x = Vec::new();
    let mut y = Vec::new();
    for p in &ps {
        let mut row = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for q in p.req["questions"].as_object().unwrap().values() {
            match q["type"].as_str().unwrap() {
                "noul" => row[1] += 1.0,
                "choice" => {
                    row[2] += 1.0;
                    row[3] += q["criteria"].as_object().unwrap().len() as f64;
                }
                _ => {
                    row[4] += 1.0;
                    row[5] += q["criteria"].as_array().unwrap().len() as f64;
                }
            }
        }
        x.push(row);
        y.push(p.output);
    }
    let (b, res, rms) = fit(&x, &y);
    println!(
        "\nadditive fit: constant {:.1}, noul {:.1}, choice {:.1} + {:.1}/option, score {:.1} + {:.1}/level; rms {rms:.2}",
        b[0], b[1], b[2], b[3], b[4], b[5]
    );
    println!(
        "residuals {}",
        res.iter()
            .map(|r| format!("{r:+.0}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

fn choice_conf(p: &[f64]) -> f64 {
    let n = p.len() as f64;
    let m = p.iter().copied().fold(0.0, f64::max);
    ((m - 1.0 / n) / (1.0 - 1.0 / n)).clamp(0.0, 1.0)
}

fn score_conf(p: &[f64]) -> f64 {
    let mut mode = 0;
    for i in 0..p.len() {
        if p[i] > p[mode] {
            mode = i;
        }
    }
    let n = p.len() as f64;
    let spread: f64 = p
        .iter()
        .enumerate()
        .map(|(i, q)| q * (i as f64 - mode as f64).abs())
        .sum();
    let c = (n - 1.0) / 2.0;
    let mad: f64 = (0..p.len()).map(|i| (i as f64 - c).abs()).sum::<f64>() / n;
    (1.0 - spread / mad).max(0.0)
}

fn r2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap()
}

/// Every published Choice and Score answer: (kind, probabilities in option
/// order, confidence, score).
fn answers() -> Vec<(String, Vec<f64>, f64, Option<f64>)> {
    let mut out = Vec::new();
    for p in pairs() {
        for (id, a) in p.resp["answers"].as_object().unwrap() {
            let kind = a["type"].as_str().unwrap().to_string();
            if kind == "noul" {
                continue;
            }
            let q = &p.req["questions"][id];
            let probs: Vec<f64> = if kind == "choice" {
                q["criteria"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|k| a["probabilities"][k].as_f64().unwrap())
                    .collect()
            } else {
                (0..q["criteria"].as_array().unwrap().len())
                    .map(|i| a["probabilities"][i.to_string()].as_f64().unwrap())
                    .collect()
            };
            out.push((
                kind,
                probs,
                a["confidence"].as_f64().unwrap(),
                a.get("score").and_then(Value::as_f64),
            ));
        }
    }
    out
}

/// Could counts k/N (summing to N) round to every published number of one
/// answer (its probabilities, its confidence, and a Score's expected level)?
fn feasible(kind: &str, pubp: &[f64], conf: f64, score: Option<f64>, n: u32) -> bool {
    let tol = 0.005 + 1e-9;
    let ranges: Vec<Vec<u32>> = pubp
        .iter()
        .map(|&p| {
            (0..=n)
                .filter(|&k| (r2(k as f64 / n as f64) - p).abs() < 1e-9)
                .collect()
        })
        .collect();
    #[allow(clippy::too_many_arguments)]
    fn rec(
        i: usize,
        left: i64,
        acc: &mut Vec<u32>,
        r: &[Vec<u32>],
        n: u32,
        kind: &str,
        conf: f64,
        score: Option<f64>,
        tol: f64,
    ) -> bool {
        if i == r.len() {
            if left != 0 {
                return false;
            }
            let p: Vec<f64> = acc.iter().map(|&k| k as f64 / n as f64).collect();
            let c = if kind == "choice" {
                choice_conf(&p)
            } else {
                score_conf(&p)
            };
            let ev: f64 = p.iter().enumerate().map(|(i, q)| i as f64 * q).sum();
            return (c - conf).abs() <= tol && score.is_none_or(|s| (ev - s).abs() <= tol);
        }
        for &k in &r[i] {
            if k as i64 <= left {
                acc.push(k);
                if rec(i + 1, left - k as i64, acc, r, n, kind, conf, score, tol) {
                    return true;
                }
                acc.pop();
            }
        }
        false
    }
    rec(
        0,
        n as i64,
        &mut Vec::new(),
        &ranges,
        n,
        kind,
        conf,
        score,
        tol,
    )
}

fn probs() {
    let ans = answers();
    let mut ok = 0;
    println!("TypeSafe's confidence formulas (system-one-adapter) against every published answer:");
    for (kind, p, c, _) in &ans {
        let mine = if kind == "choice" {
            choice_conf(p)
        } else {
            score_conf(p)
        };
        let good = (mine - c).abs() <= 0.015;
        ok += usize::from(good);
        println!(
            "  {kind:<6} {p:?} published {c:.2} formula {mine:.3} {}",
            if good { "ok" } else { "MISMATCH" }
        );
    }
    println!("  {ok} of {} within rounding\n", ans.len());
    println!("Could the probabilities be counts out of N samples?");
    let first = (2..=400u32).find(|&n| ans.iter().all(|(k, p, c, s)| feasible(k, p, *c, *s, n)));
    for n in [8u32, 16, 20, 32, 40, 50, 64, 100, 128, 200, 256] {
        let bad = ans
            .iter()
            .filter(|(k, p, c, s)| !feasible(k, p, *c, *s, n))
            .count();
        println!("  N = {n:>3}: {bad:>2} of {} answers impossible", ans.len());
    }
    match first {
        Some(n) => println!("  smallest N that fits every answer: {n}"),
        None => println!("  no N up to 400 fits every answer"),
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("input") => input(),
        Some("output") => output(),
        Some("probs") => probs(),
        _ => eprintln!(
            "usage: jevre input|output|probs  (JEVRE_TOKENIZERS names the tokenizer directory)"
        ),
    }
}
