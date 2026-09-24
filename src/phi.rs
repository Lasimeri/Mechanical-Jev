//! Intel Phi Jev's server, started and stopped for `mjev`. When the server
//! `mjev` talks to is on this machine and does not answer, `ensure` starts
//! it with Intel Phi Jev's own binary (`xks serve --detach`, which takes
//! its subject and site from that repository's `xks.conf`, loads the model,
//! puts the Phi cards to work, and returns once `/health` answers). `stop`
//! runs `xks stop`, which also gives the cards their memory back. See
//! phi.md.

use std::path::PathBuf;
use std::process::Command;

use crate::client::Client;

/// Intel Phi Jev's `xks`: `MJEV_XKS`, else the default clone's build.
pub fn xks() -> PathBuf {
    if let Some(p) = std::env::var_os("MJEV_XKS") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join("Intel Phi Jev/target/release/xks")
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
        return Err(format!(
            "Intel Phi Jev is not built at {} (cd ~/\"Intel Phi Jev\" && make build-x86), or set MJEV_XKS",
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
}
