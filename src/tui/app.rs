//! The TUI's app: what is on screen, what each key does, and the jobs that
//! run beside the screen (asking, starting and stopping the server, the
//! health check), which report over a channel so drawing never waits.
//! Drawing is view.rs; `run` is the loop. See app.md.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde_json::{Map, Value};

use super::editor::Editor;
use super::model::{state_value, Draft, Kind, QDraft};
use super::term::{self, Term};
use super::view;
use crate::client::Client;
use crate::phi::{self, Say};
use crate::protocol::{Request, Response};

/// How long a key is waited for; the spinner turns at this rate.
const POLL: Duration = Duration::from_millis(100);
/// How often the local server's health is checked while nothing runs.
const HEALTH_EVERY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Ask,
    Form,
    Examples,
    Server,
    Help,
}

/// Home's rows: the name after `/`, what it opens, its letter.
pub const MENU: [(&str, &str, char); 5] = [
    ("ask", "write a state, add questions, ask", 'a'),
    (
        "examples",
        "typesafe's published requests, jev's answers kept",
        'e',
    ),
    ("server", "start, stop, the models", 's'),
    ("help", "the keys", 'h'),
    ("quit", "", 'q'),
];

/// Where keys go on the ask screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    State,
    Questions,
}

/// The question form's fields, in `Tab` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Kind,
    Id,
    Instructions,
    Options,
}

impl Field {
    const ALL: [Field; 4] = [Field::Kind, Field::Id, Field::Instructions, Field::Options];

    fn step(self, by: isize) -> Field {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0) as isize;
        Self::ALL[(i + by).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

/// A question being written or edited.
pub struct Form {
    pub kind: Kind,
    pub id: Editor,
    pub instructions: Editor,
    pub options: Editor,
    pub field: Field,
    /// The question's index when editing one; `None` for a new one.
    pub editing: Option<usize>,
}

impl Form {
    fn of(q: &QDraft, editing: Option<usize>) -> Self {
        Self {
            kind: q.kind,
            id: Editor::with_text(&q.id, true),
            instructions: Editor::with_text(&q.instructions, false),
            options: Editor::with_text(&q.options, false),
            field: if editing.is_some() {
                Field::Instructions
            } else {
                Field::Kind
            },
            editing,
        }
    }

    fn draft(&self) -> QDraft {
        QDraft {
            id: self.id.text().trim().to_string(),
            kind: self.kind,
            instructions: self.instructions.text(),
            options: self.options.text(),
        }
    }

    /// The focused field's editor, when it is text.
    pub fn editor(&mut self) -> Option<&mut Editor> {
        match self.field {
            Field::Kind => None,
            Field::Id => Some(&mut self.id),
            Field::Instructions => Some(&mut self.instructions),
            Field::Options => Some(&mut self.options),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFor {
    Load,
    Write,
}

/// A file path asked for on the ask screen's status row.
pub struct Prompt {
    pub what: PromptFor,
    pub path: Editor,
}

/// One of TypeSafe's published requests and Jev's answers to it.
#[derive(Debug, Clone)]
pub struct Example {
    pub source: String,
    pub note: String,
    pub request: Request,
    pub answers: Map<String, Value>,
}

/// Every published example (`evidence/`).
pub fn examples() -> Vec<Example> {
    crate::evidence::all()
        .iter()
        .filter_map(|e| {
            Some(Example {
                source: e["source"].as_str().unwrap_or_default().to_string(),
                note: e["note"].as_str().unwrap_or_default().to_string(),
                request: serde_json::from_value(e["request"].clone()).ok()?,
                answers: e["response"]["answers"].as_object()?.clone(),
            })
        })
        .collect()
}

/// The last answer, with what was asked.
pub struct Asked {
    pub request: Request,
    pub response: Response,
    pub took: Duration,
}

/// What the server screen shows.
#[derive(Default)]
pub struct Server {
    /// `None` until the first check, and always for a remote server.
    pub up: Option<bool>,
    pub subject: String,
    pub models: Vec<String>,
    /// The last lines `xks` wrote, or the last error in full.
    pub log: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Asking,
    Starting,
    Stopping,
}

enum Msg {
    Phase(String),
    Health(Result<String, String>),
    Models(Result<Vec<String>, String>),
    Asked(Result<Box<Asked>, String>),
    Started(Result<String, String>),
    Stopped(Result<String, String>),
    Panicked(String),
}

pub struct App {
    pub client: Client,
    /// `host:port` when the server is this machine's, so it can be started.
    pub bind: Option<String>,
    /// Intel Phi Jev's `xks`, built or not.
    pub xks: PathBuf,
    pub screen: Screen,
    /// Where help goes back to.
    pub before_help: Screen,
    pub home_sel: usize,
    pub state: Editor,
    pub questions: Vec<QDraft>,
    pub focus: Focus,
    pub q_sel: usize,
    /// The first row of the question list shown.
    pub q_top: usize,
    pub asked: Option<Asked>,
    /// The example loaded, for Jev's numbers while it is unchanged.
    pub loaded: Option<Example>,
    pub form: Option<Form>,
    pub prompt: Option<Prompt>,
    pub last_path: String,
    pub examples: Vec<Example>,
    pub ex_sel: usize,
    pub server: Server,
    /// The job running, what it is doing, since when.
    pub job: Option<(Job, String, Instant)>,
    /// The last message, and whether it is an error.
    pub status: Option<(String, bool)>,
    pub quit: bool,
    pub spin: usize,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    health_pending: bool,
    last_health: Option<Instant>,
}

/// An editor's keys; whether the key was one.
fn edit(e: &mut Editor, k: KeyEvent) -> bool {
    let plain = !k
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match k.code {
        KeyCode::Char(c) if plain => e.insert_char(c),
        KeyCode::Enter => e.newline(),
        KeyCode::Backspace => e.backspace(),
        KeyCode::Delete => e.delete(),
        KeyCode::Left => e.left(),
        KeyCode::Right => e.right(),
        KeyCode::Up => e.up(),
        KeyCode::Down => e.down(),
        KeyCode::Home => e.home(),
        KeyCode::End => e.end(),
        KeyCode::PageUp => e.page(-10),
        KeyCode::PageDown => e.page(10),
        _ => return false,
    }
    true
}

impl App {
    pub fn new(client: Client) -> Self {
        let (tx, rx) = channel();
        Self {
            bind: phi::local_bind(&client.base),
            client,
            xks: phi::xks(),
            screen: Screen::Home,
            before_help: Screen::Home,
            home_sel: 0,
            state: Editor::new(false),
            questions: Vec::new(),
            focus: Focus::State,
            q_sel: 0,
            q_top: 0,
            asked: None,
            loaded: None,
            form: None,
            prompt: None,
            last_path: "request.json".into(),
            examples: examples(),
            ex_sel: 0,
            server: Server::default(),
            job: None,
            status: None,
            quit: false,
            spin: 0,
            tx,
            rx,
            health_pending: false,
            last_health: None,
        }
    }

    pub fn draft(&self) -> Draft {
        Draft {
            state: self.state.text(),
            questions: self.questions.clone(),
        }
    }

    /// Whether question `i` and the state are as `req` has them.
    fn unchanged(&self, req: &Request, i: usize) -> bool {
        let Some(q) = self.questions.get(i) else {
            return false;
        };
        req.state == state_value(&self.state.text())
            && req.questions.get(&q.id) == q.to_value().ok().as_ref()
    }

    /// Jev's published answer to question `i`, while the question and the
    /// state are still the loaded example's.
    pub fn jev_for(&self, i: usize) -> Option<&Value> {
        let ex = self.loaded.as_ref()?;
        let id = &self.questions.get(i)?.id;
        self.unchanged(&ex.request, i)
            .then(|| ex.answers.get(id))
            .flatten()
    }

    /// The last answer to question `i`, and whether it still answers the
    /// question as it now reads.
    pub fn answer_for(&self, i: usize) -> Option<(&Value, bool)> {
        let a = self.asked.as_ref()?;
        let v = a.response.answers.get(&self.questions.get(i)?.id)?;
        Some((v, self.unchanged(&a.request, i)))
    }

    fn info(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), false));
    }

    /// An error: its first line on the status row; the whole of a longer
    /// one (a log's tail) on the server screen.
    fn fail(&mut self, e: impl Into<String>) {
        let e = e.into();
        let first = e.lines().next().unwrap_or_default().to_string();
        if e.lines().count() > 1 {
            self.server.log = e;
            self.status = Some((format!("{first} (more on / server)"), true));
        } else {
            self.status = Some((first, true));
        }
    }

    // Keys.

    pub fn key(&mut self, k: KeyEvent) {
        if k.kind == KeyEventKind::Release {
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Char('c' | 'q') if ctrl => {
                self.quit = true;
                return;
            }
            KeyCode::F(1) if self.screen != Screen::Help => {
                self.before_help = self.screen;
                self.screen = Screen::Help;
                return;
            }
            _ => {}
        }
        if self.prompt.is_some() {
            return self.prompt_key(k);
        }
        match self.screen {
            Screen::Home => self.home_key(k),
            Screen::Ask => self.ask_key(k, ctrl),
            Screen::Form => self.form_key(k, ctrl),
            Screen::Examples => self.examples_key(k),
            Screen::Server => self.server_key(k),
            Screen::Help => {
                if matches!(k.code, KeyCode::Esc | KeyCode::F(1) | KeyCode::Enter) {
                    self.screen = self.before_help;
                }
            }
        }
    }

    /// Pasted text goes to the focused editor, if any.
    pub fn paste(&mut self, s: &str) {
        let e = if let Some(p) = &mut self.prompt {
            Some(&mut p.path)
        } else {
            match self.screen {
                Screen::Ask if self.focus == Focus::State => Some(&mut self.state),
                Screen::Form => self.form.as_mut().and_then(Form::editor),
                _ => None,
            }
        };
        if let Some(e) = e {
            e.insert_str(s);
        }
    }

    fn home_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up => self.home_sel = self.home_sel.saturating_sub(1),
            KeyCode::Down => self.home_sel = (self.home_sel + 1).min(MENU.len() - 1),
            KeyCode::Enter => self.open(self.home_sel),
            KeyCode::Char(c) => {
                if let Some(i) = MENU.iter().position(|m| m.2 == c) {
                    self.home_sel = i;
                    self.open(i);
                }
            }
            _ => {}
        }
    }

    fn open(&mut self, i: usize) {
        match MENU[i].0 {
            "ask" => self.screen = Screen::Ask,
            "examples" => self.screen = Screen::Examples,
            "server" => {
                self.screen = Screen::Server;
                self.refresh();
            }
            "help" => {
                self.before_help = Screen::Home;
                self.screen = Screen::Help;
            }
            _ => self.quit = true,
        }
    }

    fn ask_key(&mut self, k: KeyEvent, ctrl: bool) {
        match k.code {
            KeyCode::Esc => return self.screen = Screen::Home,
            KeyCode::F(5) => return self.ask(),
            KeyCode::Char('s') if ctrl => return self.ask(),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::State => Focus::Questions,
                    Focus::Questions => Focus::State,
                };
                return;
            }
            _ => {}
        }
        if self.focus == Focus::State {
            edit(&mut self.state, k);
            return;
        }
        let n = self.questions.len();
        match k.code {
            KeyCode::Up => self.q_sel = self.q_sel.saturating_sub(1),
            KeyCode::Down => self.q_sel = (self.q_sel + 1).min(n.saturating_sub(1)),
            KeyCode::Char('a') => self.open_form(None),
            KeyCode::Enter | KeyCode::Char('e') if n > 0 => self.open_form(Some(self.q_sel)),
            KeyCode::Char('d') if n > 0 => {
                let q = self.questions.remove(self.q_sel);
                self.q_sel = self.q_sel.min(self.questions.len().saturating_sub(1));
                self.info(format!("deleted {}", q.id));
            }
            KeyCode::Char('l') => self.ask_path(PromptFor::Load),
            KeyCode::Char('w') => self.ask_path(PromptFor::Write),
            KeyCode::Char('x') => self.screen = Screen::Examples,
            _ => {}
        }
    }

    fn open_form(&mut self, editing: Option<usize>) {
        let q = match editing {
            Some(i) => self.questions[i].clone(),
            None => QDraft::new(&self.draft().next_id()),
        };
        self.form = Some(Form::of(&q, editing));
        self.screen = Screen::Form;
    }

    fn form_key(&mut self, k: KeyEvent, ctrl: bool) {
        let Some(form) = self.form.as_mut() else {
            self.screen = Screen::Ask;
            return;
        };
        match k.code {
            KeyCode::Esc => {
                self.form = None;
                self.screen = Screen::Ask;
            }
            KeyCode::F(2) => self.save(),
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Tab => form.field = form.field.step(1),
            KeyCode::BackTab => form.field = form.field.step(-1),
            _ => match form.field {
                Field::Kind => match k.code {
                    KeyCode::Left => form.kind = form.kind.prev(),
                    KeyCode::Right | KeyCode::Char(' ') => form.kind = form.kind.next(),
                    KeyCode::Char('n') => form.kind = Kind::Noul,
                    KeyCode::Char('c') => form.kind = Kind::Choice,
                    KeyCode::Char('s') => form.kind = Kind::Score,
                    KeyCode::Enter | KeyCode::Down => form.field = Field::Id,
                    _ => {}
                },
                Field::Id if matches!(k.code, KeyCode::Enter | KeyCode::Down) => {
                    form.field = Field::Instructions
                }
                Field::Id if k.code == KeyCode::Up => form.field = Field::Kind,
                _ => {
                    if let Some(e) = form.editor() {
                        edit(e, k);
                    }
                }
            },
        }
    }

    /// The form's question, checked, into the list.
    fn save(&mut self) {
        let Some(form) = &self.form else { return };
        let q = form.draft();
        if let Err(e) = q.to_value() {
            return self.fail(e);
        }
        let editing = form.editing;
        if self
            .questions
            .iter()
            .enumerate()
            .any(|(i, o)| o.id == q.id && Some(i) != editing)
        {
            return self.fail(format!("id `{}` is taken", q.id));
        }
        let id = q.id.clone();
        match editing {
            Some(i) => self.questions[i] = q,
            None => {
                self.questions.push(q);
                self.q_sel = self.questions.len() - 1;
            }
        }
        self.form = None;
        self.screen = Screen::Ask;
        self.focus = Focus::Questions;
        self.info(format!("saved {id}; f5 asks"));
    }

    fn examples_key(&mut self, k: KeyEvent) {
        let n = self.examples.len();
        match k.code {
            KeyCode::Esc => self.screen = Screen::Home,
            KeyCode::Up => self.ex_sel = self.ex_sel.saturating_sub(1),
            KeyCode::Down => self.ex_sel = (self.ex_sel + 1).min(n.saturating_sub(1)),
            KeyCode::Enter if n > 0 => self.load_example(self.ex_sel),
            _ => {}
        }
    }

    /// Example `i` into ask, Jev's answers kept beside it.
    pub fn load_example(&mut self, i: usize) {
        let ex = self.examples[i].clone();
        match Draft::from_request(&ex.request) {
            Ok(d) => {
                self.set_draft(d);
                self.loaded = Some(ex);
                self.focus = Focus::Questions;
                self.screen = Screen::Ask;
                self.info("loaded; f5 asks, jev's published answer shows beside ours");
            }
            Err(e) => self.fail(e),
        }
    }

    fn set_draft(&mut self, d: Draft) {
        self.state.set_text(&d.state);
        self.questions = d.questions;
        self.q_sel = 0;
        self.q_top = 0;
        self.asked = None;
        self.loaded = None;
    }

    fn server_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => self.screen = Screen::Home,
            KeyCode::Char('s') => self.start(),
            KeyCode::Char('x') => self.stop(),
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }

    fn ask_path(&mut self, what: PromptFor) {
        self.prompt = Some(Prompt {
            what,
            path: Editor::with_text(&self.last_path, true),
        });
    }

    fn prompt_key(&mut self, k: KeyEvent) {
        let Some(p) = self.prompt.as_mut() else {
            return;
        };
        match k.code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter => {
                let (what, path) = (p.what, p.path.text().trim().to_string());
                self.prompt = None;
                if path.is_empty() {
                    return;
                }
                self.last_path = path.clone();
                match what {
                    PromptFor::Load => self.load_file(Path::new(&path)),
                    PromptFor::Write => self.write_file(Path::new(&path)),
                }
            }
            _ => {
                edit(&mut p.path, k);
            }
        }
    }

    /// A request file into ask.
    pub fn load_file(&mut self, p: &Path) {
        let r = std::fs::read_to_string(p)
            .map_err(|e| format!("{}: {e}", p.display()))
            .and_then(|t| {
                serde_json::from_str::<Request>(&t).map_err(|e| format!("{}: {e}", p.display()))
            })
            .and_then(|req| Draft::from_request(&req));
        match r {
            Ok(d) => {
                self.set_draft(d);
                self.focus = Focus::Questions;
                self.info(format!("loaded {}", p.display()));
            }
            Err(e) => self.fail(e),
        }
    }

    fn write_file(&mut self, p: &Path) {
        let r = self.draft().to_request().and_then(|req| {
            let text = serde_json::to_string_pretty(&req).map_err(|e| e.to_string())? + "\n";
            std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))
        });
        match r {
            Ok(()) => self.info(format!("wrote {}", p.display())),
            Err(e) => self.fail(e),
        }
    }

    // Jobs.

    /// Run `f` on a worker thread; what it returns (or its panic) comes
    /// back as a message.
    fn spawn(&self, f: impl FnOnce(&Sender<Msg>) -> Msg + Send + 'static) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let msg = catch_unwind(AssertUnwindSafe(|| f(&tx))).unwrap_or_else(|p| {
                Msg::Panicked(
                    term::take_worker_panic()
                        .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                        .or_else(|| p.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "a job panicked".into()),
                )
            });
            let _ = tx.send(msg);
        });
    }

    fn busy(&mut self) -> bool {
        if let Some((_, what, _)) = &self.job {
            let what = what.clone();
            self.fail(format!("busy: {what}"));
            return true;
        }
        false
    }

    /// Ask the draft, starting the local server first when it is down.
    pub fn ask(&mut self) {
        if self.busy() {
            return;
        }
        let req = match self.draft().to_request() {
            Ok(r) => r,
            Err(e) => return self.fail(e),
        };
        let (client, local) = (self.client.clone(), self.bind.is_some());
        self.job = Some((Job::Asking, "asking".into(), Instant::now()));
        self.spawn(move |tx| {
            if local && !client.healthy() {
                let _ = tx.send(Msg::Phase(
                    "starting intel phi jev (loads the model, a minute or so), then asking".into(),
                ));
                if let Err(e) = phi::ensure_as(&client, Say::Log) {
                    return Msg::Asked(Err(e));
                }
                let _ = tx.send(Msg::Phase("asking".into()));
            }
            Msg::Asked(
                client
                    .system_one(&req)
                    .map(|(response, took)| {
                        Box::new(Asked {
                            request: req,
                            response,
                            took,
                        })
                    })
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn start(&mut self) {
        let Some(bind) = self.bind.clone() else {
            return self.fail(format!(
                "{} is not this machine; nothing to start",
                self.client.base
            ));
        };
        if self.busy() {
            return;
        }
        let client = self.client.clone();
        self.job = Some((
            Job::Starting,
            "starting intel phi jev (loads the model, a minute or so)".into(),
            Instant::now(),
        ));
        self.spawn(move |_| {
            if client.healthy() {
                return Msg::Started(Ok(format!("{} already answers", client.base)));
            }
            Msg::Started(phi::start_as(&bind, Say::Log))
        });
    }

    fn stop(&mut self) {
        if self.bind.is_none() {
            return self.fail(format!(
                "{} is not this machine; nothing to stop",
                self.client.base
            ));
        }
        if self.busy() {
            return;
        }
        self.job = Some((
            Job::Stopping,
            "stopping the server, releasing the cards".into(),
            Instant::now(),
        ));
        self.spawn(|_| Msg::Stopped(phi::stop_as(Say::Log)));
    }

    /// Health (local only) and the models, now.
    fn refresh(&mut self) {
        if self.bind.is_some() && !self.health_pending {
            self.check_health();
        }
        let client = self.client.clone();
        self.spawn(move |_| {
            Msg::Models(
                client
                    .models()
                    .map(|v| {
                        v["data"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|m| m["id"].as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn check_health(&mut self) {
        self.health_pending = true;
        let client = self.client.clone();
        self.spawn(move |_| {
            Msg::Health(
                client
                    .health()
                    .map(|v| v["subject"].as_str().unwrap_or_default().to_string())
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn apply(&mut self, m: Msg) {
        match m {
            Msg::Phase(s) => {
                if let Some(j) = &mut self.job {
                    j.1 = s;
                }
            }
            Msg::Health(r) => {
                self.health_pending = false;
                self.last_health = Some(Instant::now());
                match r {
                    Ok(subject) => {
                        self.server.up = Some(true);
                        self.server.subject = subject;
                    }
                    Err(_) => {
                        self.server.up = Some(false);
                        self.server.subject.clear();
                        self.server.models.clear();
                    }
                }
            }
            Msg::Models(r) => match r {
                Ok(m) => self.server.models = m,
                Err(e) => {
                    self.server.models.clear();
                    if self.screen == Screen::Server && self.server.up != Some(false) {
                        self.fail(format!("models: {e}"));
                    }
                }
            },
            Msg::Asked(r) => {
                self.job = None;
                match r {
                    Ok(a) => {
                        self.info(format!(
                            "answered by {} in {:.1} s",
                            a.response.model,
                            a.took.as_secs_f64()
                        ));
                        self.asked = Some(*a);
                        self.last_health = None;
                    }
                    Err(e) => self.fail(e),
                }
            }
            Msg::Started(r) => {
                self.job = None;
                match r {
                    Ok(tail) => {
                        self.server.log = tail;
                        self.info("the server answers");
                        self.refresh();
                    }
                    Err(e) => self.fail(e),
                }
            }
            Msg::Stopped(r) => {
                self.job = None;
                match r {
                    Ok(tail) => {
                        self.server.log = tail;
                        self.server.up = Some(false);
                        self.server.subject.clear();
                        self.server.models.clear();
                        self.info("stopped; any cards it held are released");
                    }
                    Err(e) => self.fail(e),
                }
            }
            Msg::Panicked(s) => {
                self.job = None;
                self.health_pending = false;
                self.fail(format!("a job panicked: {s}"));
            }
        }
    }

    /// Messages from jobs, and the health check when it is due. Whether
    /// anything changed.
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        while let Ok(m) = self.rx.try_recv() {
            self.apply(m);
            changed = true;
        }
        if self.job.is_some() {
            self.spin = self.spin.wrapping_add(1);
            changed = true;
        }
        let due = self.last_health.is_none_or(|t| t.elapsed() >= HEALTH_EVERY);
        if self.bind.is_some() && self.job.is_none() && !self.health_pending && due {
            self.check_health();
        }
        changed
    }
}

/// `mjev tui`: the loop. Draws when something changed, waits up to
/// `POLL` for a key, and hands keys and pastes to the app.
pub fn run(client: Client, file: Option<PathBuf>) -> Result<(), String> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("mjev tui needs a terminal".into());
    }
    let mut app = App::new(client);
    if let Some(p) = file {
        app.load_file(&p);
        app.screen = Screen::Ask;
    }
    let mut term = Term::open().map_err(|e| format!("the terminal: {e}"))?;
    let mut dirty = true;
    while !app.quit {
        dirty |= app.tick();
        if dirty {
            let (w, h) = Term::size();
            term.draw(&view::render(&mut app, w, h))
                .map_err(|e| e.to_string())?;
            dirty = false;
        }
        if event::poll(POLL).map_err(|e| e.to_string())? {
            match event::read().map_err(|e| e.to_string())? {
                Event::Key(k) => app.key(k),
                Event::Paste(s) => app.paste(&s),
                _ => {}
            }
            dirty = true;
        }
    }
    // Back on the main screen first, so the line stays visible.
    drop(term);
    if app.bind.is_some() && app.server.up == Some(true) {
        eprintln!(
            "mjev: the server on {} is still running (on the cards it holds their memory); \
             mjev stop ends it and releases them",
            app.client.base
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(Client {
            base: "http://127.0.0.1:9".into(),
            api_key: None,
            model: "jev-latest".into(),
            attempts: 1,
            timeout: Duration::from_secs(1),
        })
    }

    fn press(a: &mut App, code: KeyCode) {
        a.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn ctrl(a: &mut App, c: char) {
        a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    }

    fn typed(a: &mut App, s: &str) {
        for c in s.chars() {
            press(a, KeyCode::Char(c));
        }
    }

    #[test]
    fn home_to_ask_to_a_saved_question() {
        let mut a = app();
        press(&mut a, KeyCode::Char('a'));
        assert_eq!(a.screen, Screen::Ask);
        typed(&mut a, "$ git status");
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Char('a'));
        assert_eq!(a.screen, Screen::Form);
        press(&mut a, KeyCode::Right); // noul to choice
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        typed(&mut a, "What kind of command?");
        press(&mut a, KeyCode::Tab);
        typed(&mut a, "vcs: version control");
        press(&mut a, KeyCode::Enter);
        typed(&mut a, "build");
        ctrl(&mut a, 's');
        assert_eq!(a.screen, Screen::Ask, "{:?}", a.status);
        let req = a.draft().to_request().unwrap();
        assert_eq!(req.state, serde_json::json!("$ git status"));
        assert_eq!(
            req.questions["q1"]["criteria"],
            serde_json::json!({"vcs": "version control", "build": null})
        );
    }

    #[test]
    fn a_bad_question_stays_in_the_form() {
        let mut a = app();
        a.screen = Screen::Ask;
        a.focus = Focus::Questions;
        press(&mut a, KeyCode::Char('a'));
        press(&mut a, KeyCode::Char('s')); // kind: score
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Tab);
        typed(&mut a, "rate it");
        press(&mut a, KeyCode::Tab);
        typed(&mut a, "only one level");
        press(&mut a, KeyCode::F(2));
        assert_eq!(a.screen, Screen::Form);
        assert!(a.status.as_ref().is_some_and(|s| s.1));
        press(&mut a, KeyCode::Esc);
        assert_eq!((a.screen, a.questions.len()), (Screen::Ask, 0));
    }

    #[test]
    fn jev_shows_only_beside_the_unchanged_example() {
        let mut a = app();
        a.screen = Screen::Examples;
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.screen, Screen::Ask);
        assert!(a.jev_for(0).is_some());
        a.focus = Focus::State;
        press(&mut a, KeyCode::Char('!'));
        assert!(a.jev_for(0).is_none());
        press(&mut a, KeyCode::Backspace);
        assert!(a.jev_for(0).is_some());
    }

    #[test]
    fn every_screen_fits_every_size() {
        let mut a = app();
        a.load_example(a.examples.len() - 1);
        a.server.log = "a\nb\nc".into();
        for &(w, h) in &[(20, 5), (40, 10), (80, 24), (200, 60)] {
            for s in [
                Screen::Home,
                Screen::Ask,
                Screen::Form,
                Screen::Examples,
                Screen::Server,
                Screen::Help,
            ] {
                a.screen = s;
                if s == Screen::Form {
                    a.open_form(Some(0));
                }
                let f = view::render(&mut a, w, h);
                assert_eq!(f.rows.len(), h as usize);
                for r in 0..h as usize {
                    assert!(f.row_width(r) <= w as usize, "{s:?} {w}x{h} row {r}");
                }
                if let Some((x, y)) = f.cursor {
                    assert!(x < w && y < h, "{s:?} {w}x{h} cursor {x},{y}");
                }
            }
        }
    }
}
