//! Mechanical Jev: System One questions to Intel Phi Jev, from Rust.
//!
//! Jev reads a state and answers typed questions: a Noul (the probability a
//! yes/no statement is true), a Choice (one option of up to 255, with every
//! option's probability and a confidence) or a Score (an ordered rubric of
//! 2 to 10 levels, with the expected level, every level's probability and a
//! confidence). No text is generated. The server is Intel Phi Jev on this
//! machine (`phi` starts it on demand); this crate is the asking side.
//!
//! - [`protocol`]: the wire format of `POST /v1/systemone`, and builders.
//! - [`client`]: the client, with the documented retries.
//! - [`phi`]: Intel Phi Jev's server started and stopped on demand.
//! - [`confidence`]: TypeSafe's confidence formulas.
//! - [`eval`]: labelled case files, scored by Jev, measured.
//! - [`corroborate`]: two recorded runs compared question by question.
//! - [`reconstruction`]: Jev's request pipeline, inferred from its
//!   documentation alone (docs/reverse-engineering.md).
//! - [`evidence`]: Jev's published answers as cases and eval rows.
//! - [`config`]: the API key and defaults from files.
//! - [`tui`]: `mjev tui`, the terminal interface (docs/tui.md).

pub mod client;
pub mod confidence;
pub mod config;
pub mod corroborate;
pub mod doctor;
pub mod eval;
pub mod evidence;
pub mod label;
pub mod phi;
pub mod policy;
pub mod protocol;
pub mod rank;
pub mod reconstruction;
pub mod tui;
