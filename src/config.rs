//! The API key and defaults from files, so no command needs them on its
//! command line. Read in order, the first to set a key winning:
//!
//! 1. the environment (anything already set is never overridden);
//! 2. `$MJEV_CONFIG`, when set;
//! 3. `mjev.local.conf` in the repository (not tracked: this machine's own);
//! 4. `~/.config/mechanical-jev/mjev.conf`;
//! 5. `mjev.conf` in the repository (tracked: the defaults).
//!
//! Each file is `KEY=VALUE` lines, `#` comments, `~/` and `$HOME` expanded,
//! optional quotes: the same file a shell can source. The repository is
//! found from the binary (`target/<profile>/mjev`). See config.md.

use std::path::{Path, PathBuf};

/// The repository this binary was built in, if it still sits in it.
pub fn repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.join("Cargo.toml").is_file() && p.join("mjev.conf").is_file())
        .map(Path::to_path_buf)
}

fn expand(v: &str, home: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(v);
    let v = match v.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => v.to_string(),
    };
    v.replace("${HOME}", home).replace("$HOME", home)
}

/// Parse one file into (key, value) pairs.
pub fn parse(text: &str, home: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            Some((k.to_string(), expand(v, home)))
        })
        .collect()
}

/// The files, most important first.
pub fn files() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("MJEV_CONFIG") {
        out.push(PathBuf::from(p));
    }
    let root = repo_root();
    if let Some(r) = &root {
        out.push(r.join("mjev.local.conf"));
    }
    out.push(Path::new(&home).join(".config/mechanical-jev/mjev.conf"));
    if let Some(r) = &root {
        out.push(r.join("mjev.conf"));
    }
    out
}

/// Put every configured key that is not already set into the environment.
/// Call first thing in `main`, before any thread exists.
pub fn load() {
    let home = std::env::var("HOME").unwrap_or_default();
    for f in files() {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for (k, v) in parse(&text, &home) {
            if std::env::var_os(&k).is_none() {
                std::env::set_var(&k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_shell_style_file() {
        let t = "# c\nTYPESAFE_DEFAULT_MODEL=jev-1.13.0\nexport TYPESAFE_BASE_URL=\"https://api.typesafe.ai\"\nbad line\nX=$HOME/y\n";
        let kv = parse(t, "/h");
        assert_eq!(
            kv,
            vec![
                ("TYPESAFE_DEFAULT_MODEL".into(), "jev-1.13.0".into()),
                ("TYPESAFE_BASE_URL".into(), "https://api.typesafe.ai".into()),
                ("X".into(), "/h/y".into()),
            ]
        );
    }
}
