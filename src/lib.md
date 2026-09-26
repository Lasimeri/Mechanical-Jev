# lib.rs: the crate

System One questions to Intel Phi Jev from Rust, the asking side only.
[`protocol`](protocol.md) has the wire types and a request builder;
[`client`](client.md) calls the server; [`phi`](phi.md) starts and stops
Intel Phi Jev's server on this machine; [`confidence`](confidence.md) has
TypeSafe's confidence formulas; [`policy`](policy.md) turns answers into
act, review or escalate by thresholds the caller owns (`mjev gate`, and
what people use Jev for: [`docs/uses.md`](../docs/uses.md));
[`label`](label.md), [`rank`](rank.md), [`fit`](fit.md) and [`guard`](guard.md)
put it to work (bulk, ranking, thresholds, a Claude Code hook);
[`eval`](eval.md) scores labelled case
files; [`corroborate`](corroborate.md) compares two recorded runs;
[`evidence`](evidence.md) turns Jev's published answers into case and row
files, the yardstick `make closeness` measures against;
[`config`](config.md) reads defaults from files;
[`reconstruction`](reconstruction.md) is Jev's request pipeline as inferred
from its documentation alone
([`docs/reverse-engineering.md`](../docs/reverse-engineering.md));
[`tui`](tui/mod.md) is the terminal interface
([`docs/tui.md`](../docs/tui.md)).
