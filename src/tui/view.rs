//! Drawing the TUI: each screen as a frame of styled rows in seaof.glass's
//! colours. Nothing here changes what the app holds but scroll positions.
//! See view.md.

use super::app::{App, Field, Focus, PromptFor, Screen, MENU};
use super::model::{answer_lines, filled, summary, Kind, Line};
use super::term::{theme, Frame, Span};

const SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// The smallest terminal drawn; anything smaller is told so.
pub const MIN: (u16, u16) = (40, 10);
/// The verse under the name, as the site has it.
const VERSE: &str = "and before the throne there was a sea of glass like unto crystal";

fn text(t: impl Into<String>) -> Span {
    Span::new(t, theme::TEXT)
}

fn dim(t: impl Into<String>) -> Span {
    Span::new(t, theme::DIM)
}

fn faint(t: impl Into<String>) -> Span {
    Span::new(t, theme::BORDER)
}

fn pad(n: usize) -> Span {
    text(" ".repeat(n))
}

/// `t` cut or padded to exactly `w` characters.
fn cell(t: &str, w: usize) -> String {
    let mut out: String = t.chars().take(w).collect();
    let n = out.chars().count();
    out.extend(std::iter::repeat_n(' ', w - n));
    out
}

/// `cell`, with `…` where it cut.
fn clip(t: &str, w: usize) -> String {
    if chars(t) > w && w > 0 {
        let mut s: String = t.chars().take(w - 1).collect();
        s.push('…');
        s
    } else {
        cell(t, w)
    }
}

fn chars(t: &str) -> usize {
    t.chars().count()
}

/// Words into lines of at most `w` characters (a longer word is cut).
fn wrap(t: &str, w: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in t.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let word: String = word.chars().take(w.max(1)).collect();
            if !line.is_empty() && chars(&line) + 1 + chars(&word) > w {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word);
        }
        out.push(line);
    }
    out
}

/// A section rule at `x`, `w` wide: `── label ────`.
fn rule(x: usize, label: &str, w: usize, lit: bool) -> Vec<Span> {
    let lab = format!("{label} ");
    let rest = w.saturating_sub(3 + chars(&lab));
    let lab = if lit { text(lab).bold() } else { dim(lab) };
    vec![pad(x), faint("── "), lab, faint("─".repeat(rest))]
}

/// A probability as a bar, `w` cells.
fn bar(p: f64, w: usize, lit: bool) -> Vec<Span> {
    let n = filled(p, w);
    let fg = if lit { theme::TEXT } else { theme::ACCENT_DIM };
    vec![Span::new("█".repeat(n), fg), faint("─".repeat(w - n))]
}

/// The server as a few words: the job when one runs, else up or down.
fn server_word(a: &App) -> Vec<Span> {
    if let Some((_, what, t0)) = &a.job {
        let s = SPIN[a.spin % SPIN.len()];
        return vec![text(format!("{s} {what} {}s", t0.elapsed().as_secs()))];
    }
    match (a.bind.is_some(), a.server.up) {
        (false, _) => vec![dim("remote")],
        (true, Some(true)) if a.server.subject.is_empty() => vec![text("● up")],
        (true, Some(true)) => vec![text("● up "), dim(a.server.subject.clone())],
        (true, Some(false)) => vec![dim("○ down")],
        (true, None) => vec![dim("· checking")],
    }
}

/// Row 0: the name, where we are, and the server on the right.
fn header(f: &mut Frame, a: &App, title: &str) {
    let w = f.width as usize;
    // Home has the name large already; the other screens carry it here.
    let mut left = vec![pad(1)];
    if !title.is_empty() {
        left.push(text("mechanical jev").bold());
        left.push(dim(format!(" / {title}")));
    }
    let right = server_word(a);
    let used: usize = left.iter().chain(&right).map(|s| chars(&s.text)).sum();
    if used + 2 <= w {
        left.push(pad(w - used - 1));
        left.extend(right);
    }
    f.set(0, left, theme::BG);
}

/// The last two rows: the status (or the prompt) and the keys.
fn footer(f: &mut Frame, a: &App, keys: &[(&str, &str)]) {
    let h = f.height;
    let w = f.width as usize;
    let status = match (&a.prompt, &a.status) {
        (Some(p), _) => {
            let label = match p.what {
                PromptFor::Load => " load from: ",
                PromptFor::Write => " write the request to: ",
                PromptFor::Answer => " write the answer to: ",
            };
            let mut ed = p.path.clone();
            let lw = chars(label);
            let shown = ed.visible(w.saturating_sub(lw + 1), 1);
            let (_, cx) = ed.cursor_on_screen();
            f.cursor = Some(((lw + cx) as u16, h - 2));
            vec![
                text(label).bold(),
                text(shown.first().cloned().unwrap_or_default()),
            ]
        }
        (None, Some((s, true))) => vec![pad(1), text(format!("! {s}")).bold()],
        // While a start runs, what the server's log says it is doing.
        (None, _) if a.job.is_some() && a.progress.is_some() => vec![
            pad(1),
            faint("› "),
            dim(a.progress.clone().unwrap_or_default()).italic(),
        ],
        (None, Some((s, false))) => vec![pad(1), dim(s.clone())],
        (None, None) => vec![],
    };
    f.set(h - 2, status, theme::BG);
    let keys = if a.prompt.is_some() {
        &[("enter", "go"), ("esc", "cancel")][..]
    } else {
        keys
    };
    f.set(h - 1, keys_row(keys, w), theme::BG);
}

/// The keys row: `(key, what)` pairs in order while they fit, `f1 help`
/// kept at the end, so a narrow terminal loses the least used first.
fn keys_row(keys: &[(&str, &str)], w: usize) -> Vec<Span> {
    let last = " · f1 help";
    let mut out = vec![pad(1)];
    let mut used = 1;
    for (i, (k, what)) in keys.iter().enumerate() {
        let sep = if i == 0 { "" } else { " · " };
        let n = chars(sep) + chars(k) + 1 + chars(what);
        if used + n + chars(last) > w {
            break;
        }
        out.push(faint(sep));
        out.push(text(*k));
        out.push(dim(format!(" {what}")));
        used += n;
    }
    let sep = if used > 1 { " · " } else { "" };
    out.extend([faint(sep), text("f1"), dim(" help")]);
    out
}

pub fn render(a: &mut App, w: u16, h: u16) -> Frame {
    let mut f = Frame::new(w, h);
    if w < MIN.0 || h < MIN.1 {
        f.set(0, vec![text("the terminal is too small")], theme::BG);
        f.set(
            1,
            vec![dim(format!("{} x {} at least", MIN.0, MIN.1))],
            theme::BG,
        );
        return f;
    }
    match a.screen {
        Screen::Home => home(&mut f, a),
        Screen::Ask => ask(&mut f, a),
        Screen::Form => form(&mut f, a),
        Screen::Examples => examples(&mut f, a),
        Screen::Server => server(&mut f, a),
        Screen::Help => help(&mut f, a),
    }
    f
}

fn home(f: &mut Frame, a: &App) {
    let (w, h) = (f.width as usize, f.height as usize);
    let cw = w.saturating_sub(4).min(72);
    let x = (w - cw) / 2;
    header(f, a, "");
    // The name, the verse (left out when the rows are few), the menu, the
    // server: as the site opens, a third of the way down.
    let mut top: Vec<Vec<Span>> = vec![vec![pad(x), text("mechanical jev").bold()]];
    let verse = wrap(VERSE, cw);
    let fixed = 1 + 2 + MENU.len() + 2;
    if fixed + verse.len() < h.saturating_sub(3) {
        top.extend(verse.into_iter().map(|l| vec![pad(x), dim(l).italic()]));
        top.push(vec![pad(x), dim("revelation 4:6")]);
    }
    let room = h.saturating_sub(3).saturating_sub(top.len() + fixed);
    let mut y = 1 + room / 3;
    for r in top {
        f.set(y as u16, r, theme::BG);
        y += 1;
    }
    y += 1;
    f.set(
        y as u16,
        rule(x, "jev on the phi cards", cw, false),
        theme::BG,
    );
    y += 1;
    for (i, (name, what, _)) in MENU.iter().enumerate() {
        let sel = i == a.home_sel;
        let bg = if sel { theme::SURFACE } else { theme::BG };
        let name_w = 12.min(cw);
        let left = format!("/ {name}");
        let desc_w = cw.saturating_sub(name_w + 2);
        let desc: String = what.chars().take(desc_w).collect();
        let gap = cw.saturating_sub(name_w + chars(&desc));
        let mut name_span = text(cell(&left, name_w)).on(bg);
        if sel {
            name_span = name_span.bold();
        }
        f.set(
            y as u16,
            vec![pad(x), name_span, pad(gap).on(bg), dim(desc).on(bg)],
            theme::BG,
        );
        y += 1;
    }
    y += 1;
    let mut line = vec![pad(x), dim("server "), text(a.client.base.clone()), pad(2)];
    line.extend(server_word(a));
    f.set(y as u16, line, theme::BG);
    y += 1;
    if a.bind.is_some() && !a.xks.is_file() {
        y += 1;
        for l in wrap(&crate::phi::not_built(&a.xks), cw) {
            if y + 2 >= h {
                break;
            }
            f.set(y as u16, vec![pad(x), dim(l)], theme::BG);
            y += 1;
        }
    }
    footer(
        f,
        a,
        &[
            ("↑ ↓ enter", "choose"),
            ("a e s h q", "jump"),
            ("ctrl+q", "quit"),
        ],
    );
}

/// The rows of the question list: spans, the colour under them, and the
/// question they belong to.
fn question_rows(a: &App, w: usize) -> Vec<(Vec<Span>, bool, usize)> {
    let mut rows = Vec::new();
    let lit = a.focus == Focus::Questions;
    let idw = a
        .questions
        .iter()
        .map(|q| chars(&q.id))
        .max()
        .unwrap_or(0)
        .clamp(4, 24);
    for (i, q) in a.questions.iter().enumerate() {
        let sel = i == a.q_sel;
        rows.push((
            vec![
                pad(1),
                text(if sel && lit { "› " } else { "  " }),
                text(clip(&q.id, idw)).bold(),
                pad(2),
                dim(cell(q.kind.name(), 6)),
                pad(2),
                text(summary(&q.instructions)),
            ],
            sel && lit,
            i,
        ));
        let jev = a.jev_for(i);
        let (answer, heading) = match a.answer_for(i) {
            Some((v, true)) => (Some((v, jev)), None),
            Some((v, false)) => (Some((v, None)), Some("changed since it was asked")),
            None => match jev {
                Some(j) => (
                    Some((j, None)),
                    Some("jev's published answer; f5 asks ours"),
                ),
                None => (None, None),
            },
        };
        if let Some(hd) = heading {
            rows.push((vec![pad(5), dim(hd).italic()], false, i));
        }
        if let Some((v, jev)) = answer {
            let lines = answer_lines(q, v, jev);
            rows.extend(answer_rows(&lines, w).into_iter().map(|r| (r, false, i)));
        }
    }
    rows
}

/// Answer lines as rows: label, bar, value, note.
fn answer_rows(lines: &[Line], w: usize) -> Vec<Vec<Span>> {
    let lw = lines
        .iter()
        .map(|l| chars(&l.label))
        .max()
        .unwrap_or(0)
        .clamp(6, 28);
    let bw = w.saturating_sub(5 + lw + 1 + 6 + 10).clamp(6, 30);
    lines
        .iter()
        .map(|l| {
            let label = cell(&l.label, lw);
            let mut r = vec![
                pad(5),
                if l.chosen {
                    text(label).bold()
                } else {
                    dim(label)
                },
                pad(1),
            ];
            if let Some(p) = l.p {
                r.extend(bar(p, bw, l.chosen));
                r.push(text(format!(" {p:.2}")));
            }
            if !l.note.is_empty() {
                r.push(dim(format!("  {}", l.note)));
            }
            r
        })
        .collect()
}

fn ask(f: &mut Frame, a: &mut App) {
    let (w, h) = (f.width as usize, f.height as usize);
    header(f, a, "ask");
    let state_h = ((h.saturating_sub(6)) / 3).clamp(3, 10);
    let lit = a.focus == Focus::State;
    f.set(1, rule(1, "state", w.saturating_sub(2), lit), theme::BG);
    let ew = w.saturating_sub(4).max(1);
    let shown = a.state.visible(ew, state_h);
    if a.state.is_empty() && !lit {
        f.set(
            2,
            vec![
                pad(2),
                dim("what jev reads: text, or a json object").italic(),
            ],
            theme::BG,
        );
    }
    for (r, l) in shown.into_iter().enumerate() {
        f.set((2 + r) as u16, vec![pad(2), text(l)], theme::BG);
    }
    if lit && a.prompt.is_none() {
        let (cy, cx) = a.state.cursor_on_screen();
        f.cursor = Some(((2 + cx) as u16, (2 + cy) as u16));
    }
    let top = 2 + state_h;
    let mut label = "questions".to_string();
    if let Some(asked) = &a.asked {
        label = format!(
            "questions · {} in {:.1} s",
            asked.response.model,
            asked.took.as_secs_f64()
        );
    }
    f.set(
        top as u16,
        rule(1, &label, w.saturating_sub(2), !lit),
        theme::BG,
    );
    let list_top = top + 1;
    let list_h = (h - 2).saturating_sub(list_top);
    let rows = question_rows(a, w);
    if rows.is_empty() {
        f.set(
            list_top as u16,
            vec![
                pad(2),
                dim("no questions yet: tab here, then a adds one").italic(),
            ],
            theme::BG,
        );
    }
    // Keep the selected question's first row in view, and as much of its
    // answer as fits.
    let first = rows.iter().position(|r| r.2 == a.q_sel).unwrap_or(0);
    let last = rows.iter().rposition(|r| r.2 == a.q_sel).unwrap_or(0);
    if first < a.q_top {
        a.q_top = first;
    } else if last >= a.q_top + list_h {
        a.q_top = first.max((last + 1).saturating_sub(list_h));
    }
    a.q_top = a.q_top.min(rows.len().saturating_sub(1));
    for (r, (spans, sel, _)) in rows.into_iter().skip(a.q_top).take(list_h).enumerate() {
        let bg = if sel { theme::SURFACE } else { theme::BG };
        f.set((list_top + r) as u16, spans, bg);
    }
    let keys: &[(&str, &str)] = match a.focus {
        Focus::State => &[("f5", "ask"), ("tab", "questions"), ("esc", "home")],
        Focus::Questions => &[
            ("f5", "ask"),
            ("a", "add"),
            ("enter", "edit"),
            ("d", "delete"),
            ("x", "examples"),
            ("tab", "state"),
            ("l", "load"),
            ("w", "write"),
            ("r", "write the answer"),
            ("u", "undo delete"),
            ("n", "new draft"),
            ("esc", "home"),
        ],
    };
    footer(f, a, keys);
}

fn form(f: &mut Frame, a: &mut App) {
    let (w, h) = (f.width as usize, f.height as usize);
    let title = match a.form.as_ref().and_then(|fm| fm.editing) {
        Some(_) => "ask / edit question",
        None => "ask / new question",
    };
    header(f, a, title);
    let Some(fm) = a.form.as_mut() else {
        return footer(f, a, &[("esc", "back")]);
    };
    f.set(
        1,
        rule(1, "question", w.saturating_sub(2), false),
        theme::BG,
    );
    let lw = 15;
    let label = |name: &str, on: bool| -> Vec<Span> {
        if on {
            vec![pad(1), text(cell(&format!("› {name}"), lw)).bold()]
        } else {
            vec![pad(1), dim(cell(&format!("  {name}"), lw))]
        }
    };
    let ew = w.saturating_sub(lw + 2).max(1);
    let mut cursor = None;

    let mut kind = label("kind", fm.field == Field::Kind);
    for k in [Kind::Noul, Kind::Choice, Kind::Score] {
        kind.push(if k == fm.kind {
            text(k.name()).bold()
        } else {
            dim(k.name())
        });
        kind.push(pad(3));
    }
    f.set(2, kind, theme::BG);

    let mut id = label("id", fm.field == Field::Id);
    id.push(text(fm.id.visible(ew, 1).concat()));
    f.set(3, id, theme::BG);
    if fm.field == Field::Id {
        let (_, cx) = fm.id.cursor_on_screen();
        cursor = Some((1 + lw + cx, 3));
    }

    let rest = (h - 2).saturating_sub(4 + 2);
    let ih = (rest / 3).clamp(1, 6);
    f.set(
        4,
        label("instructions", fm.field == Field::Instructions),
        theme::BG,
    );
    for (r, l) in fm.instructions.visible(ew, ih).into_iter().enumerate() {
        f.set((5 + r) as u16, vec![pad(1 + lw), text(l)], theme::BG);
    }
    if fm.field == Field::Instructions {
        let (cy, cx) = fm.instructions.cursor_on_screen();
        cursor = Some((1 + lw + cx, 5 + cy));
    }

    let oy = 5 + ih;
    let mut ol = label("options", fm.field == Field::Options);
    ol.push(dim(fm.kind.options_hint()).italic());
    f.set(oy as u16, ol, theme::BG);
    let oh = (h - 3).saturating_sub(oy + 1).max(1);
    for (r, l) in fm.options.visible(ew, oh).into_iter().enumerate() {
        f.set((oy + 1 + r) as u16, vec![pad(1 + lw), text(l)], theme::BG);
    }
    // The question checked as it is typed, the way saving will check it.
    let q = fm.draft();
    let n = q.options.lines().filter(|l| !l.trim().is_empty()).count();
    let check = match (q.to_value(), q.kind) {
        (Ok(_), Kind::Noul) if n == 0 => text("ready: a noul"),
        (Ok(_), Kind::Noul) => text("ready: a noul, with what yes and no mean"),
        (Ok(_), Kind::Choice) if n == 1 => text("ready: 1 option"),
        (Ok(_), Kind::Choice) => text(format!("ready: {n} options")),
        (Ok(_), Kind::Score) => text(format!("ready: {n} levels, 0 to {}", n - 1)),
        (Err(e), _) => dim(format!("not yet: {e}")),
    };
    f.set((h - 3) as u16, vec![pad(1 + lw), check.italic()], theme::BG);
    if fm.field == Field::Options {
        let (cy, cx) = fm.options.cursor_on_screen();
        cursor = Some((1 + lw + cx, oy + 1 + cy));
    }
    if let Some((x, y)) = cursor {
        if x < w && y < h - 2 {
            f.cursor = Some((x as u16, y as u16));
        }
    }
    footer(
        f,
        a,
        &[
            ("ctrl+s", "save"),
            ("tab", "next field"),
            ("← →", "kind"),
            ("esc", "cancel"),
        ],
    );
}

fn examples(f: &mut Frame, a: &App) {
    let (w, h) = (f.width as usize, f.height as usize);
    header(f, a, "examples");
    f.set(
        1,
        rule(
            1,
            "typesafe's published requests",
            w.saturating_sub(2),
            false,
        ),
        theme::BG,
    );
    // The selected request in full: where it is from, its note, its
    // questions.
    let detail: Vec<Vec<Span>> = a
        .examples
        .get(a.ex_sel)
        .map(|e| {
            let mut d = vec![vec![pad(2), dim(e.source.clone())]];
            d.extend(
                wrap(&e.note, w.saturating_sub(4))
                    .into_iter()
                    .take(2)
                    .map(|l| vec![pad(2), text(l)]),
            );
            for (id, q) in &e.request.questions {
                let instr = match &q["instructions"] {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                d.push(vec![
                    pad(2),
                    text(clip(id, 20)).bold(),
                    pad(2),
                    dim(cell(q["type"].as_str().unwrap_or_default(), 6)),
                    pad(2),
                    text(summary(&instr)),
                ]);
            }
            d
        })
        .unwrap_or_default();
    let detail_h = (detail.len() + 1).min((h - 4) / 2);
    let list_top = 2;
    let list_h = (h - 2).saturating_sub(list_top + detail_h).max(1);
    let top = a.ex_sel.saturating_sub(list_h - 1);
    for (r, (i, e)) in a
        .examples
        .iter()
        .enumerate()
        .skip(top)
        .take(list_h)
        .enumerate()
    {
        let sel = i == a.ex_sel;
        let bg = if sel { theme::SURFACE } else { theme::BG };
        let ids: Vec<&str> = e.request.questions.keys().map(String::as_str).collect();
        let kinds: Vec<&str> = e
            .request
            .questions
            .values()
            .filter_map(|q| q["type"].as_str())
            .collect();
        let state = match &e.request.state {
            serde_json::Value::String(s) => s.lines().next().unwrap_or_default().to_string(),
            v => v.to_string(),
        };
        let mut name = text(clip(&format!("/ {}", ids.join(", ")), 30));
        if sel {
            name = name.bold();
        }
        f.set(
            (list_top + r) as u16,
            vec![
                pad(1),
                name,
                pad(2),
                dim(clip(&kinds.join(", "), 20)),
                pad(2),
                dim(state),
            ],
            bg,
        );
    }
    if detail_h > 0 {
        let y = h - 2 - detail_h;
        f.set(
            y as u16,
            rule(1, "the request", w.saturating_sub(2), false),
            theme::BG,
        );
        for (r, row) in detail.into_iter().take(detail_h - 1).enumerate() {
            f.set((y + 1 + r) as u16, row, theme::BG);
        }
    }
    footer(
        f,
        a,
        &[
            ("enter", "load into ask"),
            ("↑ ↓", "choose"),
            ("esc", "home"),
        ],
    );
}

fn server(f: &mut Frame, a: &App) {
    let (w, h) = (f.width as usize, f.height as usize);
    header(f, a, "server");
    f.set(1, rule(1, "server", w.saturating_sub(2), false), theme::BG);
    let state = match (a.bind.is_some(), a.server.up) {
        (false, _) => "remote: asked directly, not started or stopped from here".to_string(),
        (true, Some(true)) => "up".into(),
        (true, Some(false)) => "down: s starts it, or asking does".into(),
        (true, None) => "checking".into(),
    };
    let built = if a.xks.is_file() {
        "built"
    } else {
        "not built"
    };
    let rows: Vec<(&str, String)> = vec![
        ("address", a.client.base.clone()),
        ("state", state),
        (
            "subject",
            if a.server.subject.is_empty() {
                "none".into()
            } else {
                a.server.subject.clone()
            },
        ),
        (
            "models",
            if a.server.models.is_empty() {
                "none".into()
            } else {
                a.server.models.join(", ")
            },
        ),
        ("xks", format!("{} ({built})", a.xks.display())),
        ("server log", crate::phi::serve_log().display().to_string()),
        ("xks said", crate::phi::log_path().display().to_string()),
    ];
    let mut y = 2;
    for (k, v) in rows {
        f.set(y, vec![pad(2), dim(cell(k, 12)), text(v)], theme::BG);
        y += 1;
    }
    y += 1;
    f.set(
        y,
        rule(1, "last run", w.saturating_sub(2), false),
        theme::BG,
    );
    y += 1;
    let lines: Vec<String> = a
        .server
        .log
        .lines()
        .flat_map(|l| wrap(l, w.saturating_sub(4)))
        .collect();
    let room = (h - 2).saturating_sub(y as usize);
    for l in lines.iter().skip(lines.len().saturating_sub(room)) {
        f.set(y, vec![pad(2), dim(l.clone())], theme::BG);
        y += 1;
    }
    footer(
        f,
        a,
        &[
            ("s", "start"),
            ("x", "stop, the cards back"),
            ("r", "refresh"),
            ("esc", "home"),
        ],
    );
}

/// Where, keys, what they do.
const KEYS: &[(&str, &str, &str)] = &[
    (
        "anywhere",
        "ctrl+c, ctrl+q",
        "quit (twice while a job runs); the draft is kept",
    ),
    ("anywhere", "f1", "help; esc goes back"),
    ("anywhere", "esc", "back (a form: cancel)"),
    ("home", "↑ ↓ enter", "choose"),
    ("home", "a e s h q", "ask, examples, server, help, quit"),
    (
        "ask",
        "f5, ctrl+s",
        "ask; starts the local server when it is down",
    ),
    ("ask", "tab", "the state or the questions"),
    ("ask, state", "keys, paste", "edit; enter is a new line"),
    ("ask, questions", "↑ ↓", "choose a question"),
    ("ask, questions", "a  enter or e  d", "add, edit, delete"),
    ("ask, questions", "l  w", "load or write a request file"),
    ("ask, questions", "r", "write the last answer to a file"),
    ("ask, questions", "u  n n", "undo a delete; a new draft"),
    ("ask, questions", "x", "examples"),
    ("form", "tab, shift+tab", "next, previous field"),
    ("form", "← →, or n c s", "the kind: noul, choice, score"),
    ("form", "ctrl+s, f2", "save (checked first)"),
    (
        "server",
        "s  x  r",
        "start, stop (the cards go back), refresh",
    ),
];

fn help(f: &mut Frame, a: &App) {
    let (w, h) = (f.width as usize, f.height as usize);
    header(f, a, "help");
    f.set(1, rule(1, "keys", w.saturating_sub(2), false), theme::BG);
    for (i, (where_, keys, does)) in KEYS.iter().enumerate().take(h - 4) {
        f.set(
            (2 + i) as u16,
            vec![
                pad(2),
                dim(cell(where_, 17)),
                text(cell(keys, 18)).bold(),
                text(*does),
            ],
            theme::BG,
        );
    }
    footer(f, a, &[("esc", "back")]);
}
