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

/// A value the way a shell reads it, for what these files use: segments
/// unquoted, `"double"` (with `$HOME` expanded, `\"` and `\\` escaped) or
/// `'single'` (literal), joined; a leading `~/` expanded; an unquoted `#`
/// at the start or after a blank starts a comment, and unquoted blanks end
/// the value. `$HOME` and `${HOME}` expand only as that whole name, so
/// `$HOMEBREW` stays as written.
fn expand(v: &str, home: &str) -> String {
    let v = v.trim_start();
    let home_at = |s: &[char], i: usize| -> Option<usize> {
        let rest: String = s[i..].iter().take(8).collect();
        if rest.starts_with("${HOME}") {
            return Some(7);
        }
        let word = rest.starts_with("$HOME")
            && !s
                .get(i + 5)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_');
        word.then_some(5)
    };
    let s: Vec<char> = v.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    if v.starts_with("~/") {
        out.push_str(home);
        i = 1;
    }
    while i < s.len() {
        match s[i] {
            '\'' => {
                i += 1;
                while i < s.len() && s[i] != '\'' {
                    out.push(s[i]);
                    i += 1;
                }
                i += 1;
            }
            '"' => {
                i += 1;
                while i < s.len() && s[i] != '"' {
                    if s[i] == '\\' && matches!(s.get(i + 1), Some('"' | '\\' | '$')) {
                        out.push(s[i + 1]);
                        i += 2;
                    } else if let Some(n) = (s[i] == '$').then(|| home_at(&s, i)).flatten() {
                        out.push_str(home);
                        i += n;
                    } else {
                        out.push(s[i]);
                        i += 1;
                    }
                }
                i += 1;
            }
            c if c.is_whitespace() => break,
            '#' if i == 0 => break,
            '$' => match home_at(&s, i) {
                Some(n) => {
                    out.push_str(home);
                    i += n;
                }
                None => {
                    out.push('$');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Parse one file into (key, value) pairs.
pub fn parse(text: &str, home: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let l = l
                .strip_prefix("export")
                .filter(|r| r.starts_with(char::is_whitespace))
                .map_or(l, str::trim_start);
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

    #[test]
    fn values_read_as_a_shell_reads_them() {
        let v = |s: &str| super::expand(s, "/h");
        assert_eq!(v("0  # off"), "0");
        assert_eq!(v("\"v\" # c"), "v");
        assert_eq!(v("'$HOME/x'"), "$HOME/x");
        assert_eq!(v("$HOMEBREW_PREFIX/bin"), "$HOMEBREW_PREFIX/bin");
        assert_eq!(v("${HOME}/a"), "/h/a");
        assert_eq!(
            v("\"$HOME/Intel Phi Jev\"/target"),
            "/h/Intel Phi Jev/target"
        );
        assert_eq!(v("\"a \\\"q\\\" b\""), "a \"q\" b");
        assert_eq!(v("~/m"), "/h/m");
        assert_eq!(v(""), "");
        let kv = super::parse("export\tA=1\nexportB=2\n", "/h");
        assert_eq!(
            kv,
            vec![("A".into(), "1".into()), ("exportB".into(), "2".into())]
        );
    }
}
