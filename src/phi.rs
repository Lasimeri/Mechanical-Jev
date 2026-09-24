//! Intel Phi Jev's server, started and stopped for `mjev`. When the server
//! `mjev` talks to is on this machine and does not answer, `ensure` starts
//! it with Intel Phi Jev's own binary (`xks serve --detach`, which takes
//! its subject and site from that repository's `xks.conf`, loads the model,
//! puts the Phi cards to work, and returns once `/health` answers). `stop`
//! runs `xks stop`, which also gives the cards their memory back. See
//! phi.md.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::client::Client;

/// Intel Phi Jev's directory names: its GitHub clone's and the spaced one.
pub const SIBLING_NAMES: [&str; 2] = ["Intel-Phi-Jev", "Intel Phi Jev"];

/// Where `xks` sits in an Intel Phi Jev checkout.
const XKS_IN_CHECKOUT: &str = "target/release/xks";

/// Intel Phi Jev's `xks`: `MJEV_XKS`, else `xks` on PATH, else the build
/// in a checkout next to this one under either name, else in `$HOME`.
/// When none is built, the spaced name next to this checkout (so an error
/// names a path).
pub fn xks() -> PathBuf {
    if let Some(p) = std::env::var_os("MJEV_XKS") {
        return PathBuf::from(p);
    }
    if let Some(p) = on_path("xks") {
        return p;
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut bases: Vec<PathBuf> = here.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(PathBuf::from(home));
    }
    find_sibling(&bases, &SIBLING_NAMES, XKS_IN_CHECKOUT)
        .map(|d| d.join(XKS_IN_CHECKOUT))
        .unwrap_or_else(|| here.with_file_name(SIBLING_NAMES[1]).join(XKS_IN_CHECKOUT))
}

/// The first `base/name` holding `probe`, bases in order, names in order.
pub fn find_sibling(bases: &[PathBuf], names: &[&str], probe: &str) -> Option<PathBuf> {
    bases
        .iter()
        .flat_map(|b| names.iter().map(move |n| b.join(n)))
        .find(|d| d.join(probe).exists())
}

/// An executable file named `name` in a `PATH` directory.
fn on_path(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// `host:port` of the client's base URL, when it is this machine.
fn local_bind(base: &str) -> Option<String> {
    let rest = base.strip_prefix("http://")?;
    let hostport = rest.split('/').next()?;
    let host = hostport.split(':').next()?;
    matches!(host, "127.0.0.1" | "localhost").then(|| hostport.to_string())
}

/// The server answering, started if it is local and down (unless
/// `MJEV_AUTOSTART=0`).
pub fn ensure(client: &Client) -> Result<(), String> {
    if client.healthy() {
        return Ok(());
    }
    let Some(bind) = local_bind(&client.base) else {
        return Err(format!("{} does not answer", client.base));
    };
    if std::env::var("MJEV_AUTOSTART").as_deref() == Ok("0") {
        return Err(format!(
            "{} does not answer (MJEV_AUTOSTART=0; start it with mjev serve)",
            client.base
        ));
    }
    start(&bind)
}

/// Start the server on `bind` (`host:port`).
pub fn start(bind: &str) -> Result<(), String> {
    let bin = xks();
    if !bin.is_file() {
        let checkout = bin
            .ancestors()
            .nth(3)
            .map_or_else(|| "Intel Phi Jev".into(), |p| p.display().to_string());
        return Err(format!(
            "Intel Phi Jev is not built at {} (clone github.com/Lasimeri/Intel-Phi-Jev next to this \
             checkout, then: cd \"{checkout}\" && make build-x86), or set MJEV_XKS",
            bin.display()
        ));
    }
    eprintln!("mjev: starting Intel Phi Jev on {bind} (loads the model; a minute or so)");
    let status = Command::new(&bin)
        .args(["serve", "--detach", "--bind", bind])
        .stdout(std::process::Stdio::from(std::io::stderr()))
        .status()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(
            "Intel Phi Jev's server did not start (see its log: $XDG_RUNTIME_DIR/xks/serve.log)"
                .into(),
        )
    }
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
    fn xks_is_found_under_either_name_nearest_base_first() {
        use super::{find_sibling, SIBLING_NAMES, XKS_IN_CHECKOUT};
        let tmp = std::env::temp_dir().join(format!("mjev-sibling-{}", std::process::id()));
        let (near, home) = (tmp.join("near"), tmp.join("home"));
        let plant = |base: &std::path::Path, name: &str| {
            let bin = base.join(name).join(XKS_IN_CHECKOUT);
            std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
            std::fs::write(bin, "").unwrap();
        };
        let bases = [near.clone(), home.clone()];
        assert_eq!(find_sibling(&bases, &SIBLING_NAMES, XKS_IN_CHECKOUT), None);
        plant(&home, SIBLING_NAMES[1]);
        assert_eq!(
            find_sibling(&bases, &SIBLING_NAMES, XKS_IN_CHECKOUT),
            Some(home.join(SIBLING_NAMES[1]))
        );
        plant(&near, SIBLING_NAMES[0]);
        assert_eq!(
            find_sibling(&bases, &SIBLING_NAMES, XKS_IN_CHECKOUT),
            Some(near.join(SIBLING_NAMES[0]))
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
