//! The terminal interface (`mjev tui`), themed after seaof.glass. Built in
//! stages: the editing, the model and the terminal are here and tested;
//! the app loop that joins them (screens, keys, background jobs) is next
//! (docs/tui.md). See mod.md.

pub mod editor;
pub mod model;
pub mod term;
