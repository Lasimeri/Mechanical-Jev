# lib.rs: the crate

System One questions to Intel Phi Jev from Rust, the asking side only.
[`protocol`](protocol.md) has the wire types and a request builder;
[`client`](client.md) calls the server; [`phi`](phi.md) starts and stops
Intel Phi Jev's server on this machine; [`confidence`](confidence.md) has
TypeSafe's confidence formulas; [`eval`](eval.md) scores labelled case
files; [`corroborate`](corroborate.md) compares two recorded runs;
[`config`](config.md) reads defaults from files.
