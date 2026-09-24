# config.rs: the key and defaults from files

First to set a key wins: the environment, `$MJEV_CONFIG`, `mjev.local.conf`
(not tracked: the API key goes here), `~/.config/mechanical-jev/mjev.conf`,
`mjev.conf` (tracked defaults). The files are what a shell can source:
`KEY=VALUE`, `#` comments, optional `export` and quotes, `~/` and `$HOME`
expanded. The repository is found from the binary's path.

Values are read as a shell reads them, for what these files use:
unquoted, `"double"` (with `$HOME` expanded and `\"`, `\\`, `\$` escaped)
or `'single'` (literal) segments, joined; a leading `~/`; an unquoted `#`
at the start or after a blank starts a comment, and unquoted blanks end
the value. `$HOME` and `${HOME}` expand only as that whole name. Before
2026-09-24 `MJEV_AUTOSTART=0  # off` read as `0  # off`, `'$HOME'` was
expanded, and `$HOMEBREW_PREFIX` became the home directory plus `BREW_PREFIX`.
