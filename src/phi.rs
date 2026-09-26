//! Intel Phi Jev's server, started and stopped for `mjev`. When the server
//! `mjev` talks to is on this machine and does not answer, `ensure` starts
//! it with Intel Phi Jev's own binary (`xks serve --detach`, which takes
//! its subject and site from that repository's `xks.conf`, loads the model,
//! puts the Phi cards to work, and returns once `/health` answers). `stop`
//! runs `xks stop`, which also gives the cards their memory back. See
//! phi.md.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::client::Client;

/// Intel Phi Jev's directory names: its GitHub clone's and the spaced one.
pub const SIBLING_NAMES: [&str; 2] = ["Intel-Phi-Jev", "Intel Phi Jev"];

/// Where `xks` sits in an Intel Phi Jev checkout.
const XKS_IN_CHECKOUT: &str = "target/release/xks";

/// Intel Phi Jev's `xks`: `MJEV_XKS`, else `xks` on PATH, else the build
/// in a checkout next to this one under either name, else in `$HOME`.
/// When none is built, where it would be in the first checkout found (so
/// the error names the directory to build in), else next to this one.
pub fn xks() -> PathBuf {
    if let Some(p) = std::env::var_os("MJEV_XKS") {
        return PathBuf::from(p);
    }
    // Through a link (`make install` puts one in ~/.local/bin), the file it
    // links: anything derived from the path (the checkout) is then right.
    if let Some(p) = on_path("xks") {
        return p.canonicalize().unwrap_or(p);
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut bases: Vec<PathBuf> = here.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(PathBuf::from(home));
    }
    checkout(&bases)
        .unwrap_or_else(|| here.with_file_name(SIBLING_NAMES[1]))
        .join(XKS_IN_CHECKOUT)
}

/// The Intel Phi Jev checkout to use: the first with `xks` built, else the
/// first at all (it has `Cargo.toml`), bases in order, names in order.
pub fn checkout(bases: &[PathBuf]) -> Option<PathBuf> {
    find_sibling(bases, &SIBLING_NAMES, XKS_IN_CHECKOUT)
        .or_else(|| find_sibling(bases, &SIBLING_NAMES, "Cargo.toml"))
}

/// The first `base/name` holding `probe`, bases in order, names in order.
pub fn find_sibling(bases: &[PathBuf], names: &[&str], probe: &str) -> Option<PathBuf> {
    bases
        .iter()
        .flat_map(|b| names.iter().map(move |n| b.join(n)))
        .find(|d| d.join(probe).exists())
}

/// An executable file named `name` in a `PATH` directory.
pub fn on_path(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// `host:port` of the client's base URL, when it is this machine.
pub fn local_bind(base: &str) -> Option<String> {
    let rest = base.strip_prefix("http://")?;
    let hostport = rest.split('/').next()?;
    let host = hostport.split(':').next()?;
    matches!(host, "127.0.0.1" | "localhost").then(|| hostport.to_string())
}

/// The server answering, started if it is local and down (unless
/// `MJEV_AUTOSTART=0`).
pub fn ensure(client: &Client) -> Result<(), String> {
    ensure_as(client, Say::Terminal).map(drop)
}

/// `ensure`, with `xks`'s messages sent where `say` says. Returns what the
/// log holds when a start wrote one (else nothing).
pub fn ensure_as(client: &Client, say: Say) -> Result<String, String> {
    // A remote server is asked directly: `/health` is xks's, not part of
    // the System One API, and nothing here can start a remote one.
    let Some(bind) = local_bind(&client.base) else {
        return Ok(String::new());
    };
    if client.healthy() {
        return Ok(String::new());
    }
    if std::env::var("MJEV_AUTOSTART").as_deref() == Ok("0") {
        return Err(format!(
            "{} does not answer (MJEV_AUTOSTART=0; start it with mjev serve)",
            client.base
        ));
    }
    start_as(&bind, say)
}

/// `mjev serve`: start the local server whatever `MJEV_AUTOSTART` says
/// (it governs starting on demand, not being asked to).
pub fn serve(client: &Client) -> Result<(), String> {
    let Some(bind) = local_bind(&client.base) else {
        return Err(format!(
            "{} is not this machine; nothing to start",
            client.base
        ));
    };
    if client.healthy() {
        eprintln!("mjev: {} already answers", client.base);
        return Ok(());
    }
    start(&bind)
}

/// Where `xks`'s own messages go: the terminal (`mjev`'s commands), or a
/// log whose last lines come back (the TUI, whose screen they would tear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Say {
    Terminal,
    Log,
}

/// The log `Say::Log` writes, fresh for each run of `xks`:
/// `$XDG_RUNTIME_DIR/mjev/xks.log`, else in the temporary directory.
pub fn log_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("mjev")
        .join("xks.log")
}

/// Intel Phi Jev's detached server's own log, which `xks serve --detach`
/// writes afresh at each start: `$XDG_RUNTIME_DIR/xks/serve.log`, else in
/// the temporary directory (xks's `run_dir`). The TUI shows its last line
/// as a start's progress.
pub fn serve_log() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("xks")
        .join("serve.log")
}

/// The last `n` lines of the log.
fn log_tail(n: usize) -> String {
    let text = std::fs::read_to_string(log_path()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// `xks` with `args`, its output where `say` says. A file, not a pipe, for
/// the log: `status` returns when `xks` does, even if something it
/// started keeps an inherited descriptor open.
fn run_xks(args: &[&str], say: Say) -> Result<bool, String> {
    let bin = xks();
    let mut cmd = Command::new(&bin);
    cmd.args(args).stdin(Stdio::null());
    match say {
        // As `mjev` always ran it: `xks`'s stdout onto stderr, so a
        // command's own output stays clean.
        Say::Terminal => {
            cmd.stdout(Stdio::from(std::io::stderr()));
        }
        Say::Log => {
            let p = log_path();
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            let out = std::fs::File::create(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let err = out.try_clone().map_err(|e| e.to_string())?;
            cmd.stdout(out).stderr(err);
        }
    }
    cmd.status()
        .map(|s| s.success())
        .map_err(|e| format!("{}: {e}", bin.display()))
}

/// An error, with the log's last lines under it when there is a log.
fn with_tail(msg: &str, say: Say) -> String {
    match say {
        Say::Terminal => msg.into(),
        Say::Log => format!("{msg}\n{}", log_tail(6)),
    }
}

/// Start the server on `bind` (`host:port`).
pub fn start(bind: &str) -> Result<(), String> {
    start_as(bind, Say::Terminal).map(drop)
}

/// `start`, with `xks`'s messages sent where `say` says.
pub fn start_as(bind: &str, say: Say) -> Result<String, String> {
    let bin = xks();
    if !bin.is_file() {
        return Err(not_built(&bin));
    }
    if say == Say::Terminal {
        eprintln!("mjev: starting Intel Phi Jev on {bind} (loads the model; a minute or so)");
    }
    if run_xks(&["serve", "--detach", "--bind", bind], say)? {
        Ok(if say == Say::Log {
            log_tail(6)
        } else {
            String::new()
        })
    } else {
        Err(with_tail(
            "Intel Phi Jev's server did not start (see its log: $XDG_RUNTIME_DIR/xks/serve.log)",
            say,
        ))
    }
}

/// Why `bin` cannot run, and how to build it.
pub fn not_built(bin: &Path) -> String {
    let checkout = bin
        .ancestors()
        .nth(3)
        .map_or_else(|| "Intel Phi Jev".into(), |p| p.display().to_string());
    format!(
        "Intel Phi Jev is not built at {} (clone github.com/Lasimeri/Intel-Phi-Jev next to this \
         checkout, then: cd \"{checkout}\" && make build-x86), or set MJEV_XKS",
        bin.display()
    )
}

/// Stop the server and release the cards.
pub fn stop() -> Result<(), String> {
    let bin = xks();
    let status = Command::new(&bin)
        .arg("stop")
        .status()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err("xks stop failed".into())
    }
}

/// `stop`, with `xks`'s messages in the log; returns its last lines.
pub fn stop_as(say: Say) -> Result<String, String> {
    if say == Say::Terminal {
        return stop().map(|_| String::new());
    }
    if run_xks(&["stop"], say)? {
        Ok(log_tail(6))
    } else {
        Err(with_tail("xks stop failed", say))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_this_machine_is_started() {
        assert_eq!(
            super::local_bind("http://127.0.0.1:8090"),
            Some("127.0.0.1:8090".into())
        );
        assert_eq!(
            super::local_bind("http://localhost:9000/"),
            Some("localhost:9000".into())
        );
        assert_eq!(super::local_bind("https://api.typesafe.ai"), None);
    }

    #[test]
    fn xks_is_found_under_either_name_built_first_then_nearest() {
        use super::{checkout, SIBLING_NAMES, XKS_IN_CHECKOUT};
        let tmp = std::env::temp_dir().join(format!("mjev-sibling-{}", std::process::id()));
        let (near, home) = (tmp.join("near"), tmp.join("home"));
        let clone = |base: &std::path::Path, name: &str| {
            std::fs::create_dir_all(base.join(name)).unwrap();
            std::fs::write(base.join(name).join("Cargo.toml"), "").unwrap();
        };
        let build = |base: &std::path::Path, name: &str| {
            let bin = base.join(name).join(XKS_IN_CHECKOUT);
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            std::fs::write(bin, "").unwrap();
        };
        let bases = [near.clone(), home.clone()];
        assert_eq!(checkout(&bases), None);
        // A fresh, unbuilt clone: the error must name it, not a path that
        // does not exist.
        clone(&near, SIBLING_NAMES[0]);
        assert_eq!(checkout(&bases), Some(near.join(SIBLING_NAMES[0])));
        // A built checkout wins over an unbuilt nearer one.
        clone(&home, SIBLING_NAMES[1]);
        build(&home, SIBLING_NAMES[1]);
        assert_eq!(checkout(&bases), Some(home.join(SIBLING_NAMES[1])));
        // Both built: the nearer one.
        build(&near, SIBLING_NAMES[0]);
        assert_eq!(checkout(&bases), Some(near.join(SIBLING_NAMES[0])));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
