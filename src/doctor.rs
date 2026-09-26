//! `mjev doctor`: the family's one setup check. What this machine has of
//! what asking a question needs, from `mjev` itself through Intel Phi
//! Jev's `xks` (whose own `xks doctor` covers the rest of the chain, down
//! to the cards) to the server, with the fix for what is missing. It
//! starts nothing: no server, no card. `--fix` does what is a build or a
//! link. See doctor.md.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::client::Client;
use crate::phi;

/// How a finding bears on asking a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Note,
    Missing,
}

#[derive(Debug)]
pub struct Finding {
    pub level: Level,
    pub what: &'static str,
    pub detail: String,
    pub fix: Option<String>,
}

#[derive(Debug, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    fn add(&mut self, level: Level, what: &'static str, detail: String, fix: Option<String>) {
        self.findings.push(Finding {
            level,
            what,
            detail,
            fix,
        });
    }

    fn ok(&mut self, what: &'static str, detail: impl Into<String>) {
        self.add(Level::Ok, what, detail.into(), None);
    }

    fn note(&mut self, what: &'static str, detail: impl Into<String>, fix: Option<String>) {
        self.add(Level::Note, what, detail.into(), fix);
    }

    fn missing(&mut self, what: &'static str, detail: impl Into<String>, fix: impl Into<String>) {
        self.add(Level::Missing, what, detail.into(), Some(fix.into()));
    }

    pub fn ready(&self) -> bool {
        !self.findings.iter().any(|f| f.level == Level::Missing)
    }

    /// The findings, one line each and the fix under it.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for f in &self.findings {
            let tag = match f.level {
                Level::Ok => "ok  ",
                Level::Note => "note",
                Level::Missing => "MISS",
            };
            out.push_str(&format!("  {tag}  {:<12} {}\n", f.what, f.detail));
            if let Some(fix) = &f.fix {
                out.push_str(&format!("        {:<12} fix: {fix}\n", ""));
            }
        }
        out
    }
}

/// What is at a link's place, against the file it should point at.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkState {
    Absent,
    /// A link to `target` (through any links in between).
    Ours,
    /// A link to something else that exists: another checkout's build.
    Other(PathBuf),
    /// A link to nothing.
    Dangling(PathBuf),
    /// A file or directory, not a link: never replaced.
    File,
}

pub fn link_state(link: &Path, target: &Path) -> LinkState {
    let Ok(meta) = std::fs::symlink_metadata(link) else {
        return LinkState::Absent;
    };
    if !meta.file_type().is_symlink() {
        return LinkState::File;
    }
    let to = std::fs::read_link(link).unwrap_or_default();
    match (link.canonicalize(), target.canonicalize()) {
        (Ok(a), Ok(b)) if a == b => LinkState::Ours,
        (Err(_), _) => LinkState::Dangling(to),
        _ => LinkState::Other(to),
    }
}

/// Point `link` at `target`: created when absent, replaced when dangling or
/// linking another build, left alone (an error) when a file is there.
pub fn place_link(link: &Path, target: &Path) -> Result<String, String> {
    let at = |e: std::io::Error| format!("{}: {e}", link.display());
    let before = link_state(link, target);
    match &before {
        LinkState::Ours => return Ok(format!("{} already links here", link.display())),
        LinkState::File => {
            return Err(format!(
                "{} is a file, not a link: left alone (move it, then run this again)",
                link.display()
            ))
        }
        LinkState::Absent => {
            if let Some(d) = link.parent() {
                std::fs::create_dir_all(d).map_err(at)?;
            }
        }
        LinkState::Other(_) | LinkState::Dangling(_) => std::fs::remove_file(link).map_err(at)?,
    }
    std::os::unix::fs::symlink(target, link).map_err(at)?;
    Ok(match before {
        LinkState::Other(p) => format!(
            "{} now links here (it linked {})",
            link.display(),
            p.display()
        ),
        LinkState::Dangling(p) => format!(
            "{} now links here (it linked {}, which is gone)",
            link.display(),
            p.display()
        ),
        _ => format!("{} links here", link.display()),
    })
}

fn path_has(dir: &Path) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d == dir))
}

/// The last `n` non-empty lines of a command's output.
fn tail(bytes: &[u8], n: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n        ")
}

/// The Intel Phi Jev checkout an `xks` path belongs to: the directory
/// holding `target/release/xks` when it has a `Cargo.toml`.
fn checkout_of(xks: &Path) -> Option<PathBuf> {
    let c = xks.ancestors().nth(3)?;
    c.join("Cargo.toml").is_file().then(|| c.to_path_buf())
}

/// Build `xks` in `checkout` (`make build-x86`), first checking for the
/// llama.cpp build it links, which a cargo error would only hint at.
fn build_xks(checkout: &Path) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let src = std::env::var("LLAMA_CPP_DIR").unwrap_or(format!("{home}/llama.cpp"));
    let lib = std::env::var("LLAMA_BUILD_DIR").unwrap_or(format!("{src}/build-native/bin"));
    if !Path::new(&lib).join("libllama.so").exists() {
        return Err(format!(
            "xks links llama.cpp's x86-64 build, and {lib}/libllama.so is missing: build llama.cpp \
             there (Intel Phi Jev's build.md), or set LLAMA_CPP_DIR"
        ));
    }
    let out = Command::new("make")
        .arg("build-x86")
        .current_dir(checkout)
        .output()
        .map_err(|e| format!("make: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "the build failed:\n        {}",
            tail(&[out.stdout, out.stderr].concat(), 6)
        ))
    }
}

/// Everything, in the order a question needs it; then `xks doctor`'s own
/// report, relayed. Returns the whole text and whether a question can be
/// asked.
pub fn run(client: &Client, fix: bool, prefix: &Path) -> (String, bool) {
    let mut r = Report::default();
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .unwrap_or_default();
    let bin = prefix.join("bin");

    // mjev itself.
    let found = phi::on_path("mjev").and_then(|p| p.canonicalize().ok());
    if found.as_deref() == Some(exe.as_path()) {
        r.ok("mjev", format!("{} (on PATH)", exe.display()));
    } else if fix {
        match place_link(&bin.join("mjev"), &exe) {
            Ok(done) => r.ok("mjev", done),
            Err(e) => r.note("mjev", e, None),
        }
    } else {
        r.note(
            "mjev",
            format!("{} is not on PATH", exe.display()),
            Some("make install, or mjev doctor --fix".into()),
        );
    }
    if !path_has(&bin) {
        r.note(
            "PATH",
            format!(
                "{} is not on PATH, so a link there is not found",
                bin.display()
            ),
            Some(format!(
                "fish: fish_add_path {0}; bash: export PATH=\"{0}:$PATH\" in ~/.bashrc",
                bin.display()
            )),
        );
    }

    // Where questions go.
    let local = phi::local_bind(&client.base);
    match &local {
        Some(bind) => r.ok(
            "questions",
            format!("go to {}: Intel Phi Jev on this machine ({bind}), no key needed", client.base),
        ),
        None if client.api_key.is_some() => {
            r.ok("questions", format!("go to {} (hosted; TYPESAFE_API_KEY set)", client.base))
        }
        None => r.missing(
            "questions",
            format!("{} is not this machine and TYPESAFE_API_KEY is not set", client.base),
            "TYPESAFE_API_KEY=... in mjev.local.conf, or TYPESAFE_BASE_URL=http://127.0.0.1:8090 for Intel Phi Jev",
        ),
    }
    if local.is_none() {
        let ready = r.ready();
        return (r.text() + &summary(ready, "the hosted Jev"), ready);
    }

    // Intel Phi Jev's xks.
    let xks = phi::xks();
    let xks = xks.canonicalize().unwrap_or(xks);
    if !xks.is_file() {
        match (fix, checkout_of(&xks)) {
            (true, Some(c)) => match build_xks(&c) {
                Ok(()) => r.ok("xks build", format!("built {}", xks.display())),
                Err(e) => r.missing("xks build", e, format!("cd \"{}\" && make build-x86", c.display())),
            },
            (_, found) => r.missing(
                "xks build",
                format!("xks is not built at {}", xks.display()),
                match found {
                    Some(c) => format!("cd \"{}\" && make build-x86 (or mjev doctor --fix)", c.display()),
                    None => "clone github.com/Lasimeri/Intel-Phi-Jev next to this checkout, then make build-x86 in it (or set MJEV_XKS)".into(),
                },
            ),
        }
    } else {
        r.ok("xks build", xks.display().to_string());
    }
    let mut text = r.text();
    let mut ready = r.ready();
    if xks.is_file() {
        let mut cmd = Command::new(&xks);
        cmd.arg("doctor").arg("--prefix").arg(prefix);
        if fix {
            cmd.arg("--fix");
        }
        match cmd.output() {
            Ok(o) => {
                text.push_str(&String::from_utf8_lossy(&o.stdout));
                ready &= o.status.success();
            }
            Err(e) => {
                text.push_str(&format!(
                    "  MISS  xks doctor   could not run {}: {e}\n",
                    xks.display()
                ));
                ready = false;
            }
        }
    }

    // The server, now: up, or started when a question needs it.
    let mut s = Report::default();
    match client.health() {
        Ok(h) => s.ok(
            "server now",
            format!(
                "{} answers: {} on {}",
                client.base,
                h["subject"].as_str().unwrap_or("?"),
                h["site"].as_str().unwrap_or("?")
            ),
        ),
        Err(_) if std::env::var("MJEV_AUTOSTART").as_deref() == Ok("0") => s.note(
            "server now",
            format!("{} is down and MJEV_AUTOSTART=0", client.base),
            Some("mjev serve".into()),
        ),
        Err(_) => s.ok(
            "server now",
            format!(
                "{} is down; mjev starts it with the first question",
                client.base
            ),
        ),
    }
    text.push_str(&s.text());
    (text.clone() + &summary(ready, "Intel Phi Jev"), ready)
}

fn summary(ready: bool, to: &str) -> String {
    if ready {
        format!("ready: mjev can ask {to} (mjev opens the TUI; mjev query --help)\n")
    } else {
        "not ready: fix what says MISS above (mjev doctor --fix does the builds and links)\n".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_placed_only_where_no_file_would_be_lost() {
        let dir = std::env::temp_dir().join(format!("mjev-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("mjev-build");
        std::fs::write(&target, "").unwrap();
        let other = dir.join("other-build");
        std::fs::write(&other, "").unwrap();
        let link = dir.join("bin/mjev");

        assert_eq!(link_state(&link, &target), LinkState::Absent);
        place_link(&link, &target).unwrap();
        assert_eq!(link_state(&link, &target), LinkState::Ours);

        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&other, &link).unwrap();
        assert_eq!(link_state(&link, &target), LinkState::Other(other.clone()));
        assert!(place_link(&link, &target).unwrap().contains("other-build"));

        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(dir.join("gone"), &link).unwrap();
        assert!(matches!(link_state(&link, &target), LinkState::Dangling(_)));
        place_link(&link, &target).unwrap();
        assert_eq!(link_state(&link, &target), LinkState::Ours);

        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, "a user's own script").unwrap();
        assert!(place_link(&link, &target).is_err());
        assert_eq!(
            std::fs::read_to_string(&link).unwrap(),
            "a user's own script"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_checkout_is_the_directory_holding_the_build() {
        let dir = std::env::temp_dir().join(format!("mjev-co-{}", std::process::id()));
        let bin = dir.join("target/release/xks");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        assert_eq!(checkout_of(&bin), None);
        std::fs::write(dir.join("Cargo.toml"), "").unwrap();
        assert_eq!(checkout_of(&bin), Some(dir.clone()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
