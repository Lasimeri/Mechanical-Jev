# config.rs: the key and defaults from files

First to set a key wins: the environment, `$MJEV_CONFIG`, `mjev.local.conf`
(not tracked: the API key goes here), `~/.config/mechanical-jev/mjev.conf`,
`mjev.conf` (tracked defaults). The files are what a shell can source:
`KEY=VALUE`, `#` comments, optional `export` and quotes, `~/` and `$HOME`
expanded. The repository is found from the binary's path.
