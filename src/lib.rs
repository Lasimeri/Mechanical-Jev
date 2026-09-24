//! Mechanical Jev: TypeSafe's Jev, the first System One model, from Rust.
//!
//! Jev reads a state and answers typed questions: a Noul (the probability a
//! yes/no statement is true), a Choice (one option of up to 255, with every
//! option's probability and a confidence) or a Score (an ordered rubric of
//! 2 to 10 levels, with the expected level, every level's probability and a
//! confidence). It generates no text. Jev runs only on TypeSafe's servers;
//! this crate is the client side, nothing else.
//!
//! - [`protocol`]: the wire format of `POST /v1/systemone`, and builders.
//! - [`client`]: the API client, with the documented retries.
//! - [`confidence`]: TypeSafe's confidence formulas.
//! - [`eval`]: labelled case files, scored by Jev, measured.
//! - [`corroborate`]: two recorded runs compared question by question.
//! - [`config`]: the API key and defaults from files.

pub mod client;
pub mod confidence;
pub mod config;
pub mod corroborate;
pub mod eval;
pub mod protocol;
