//! What a project knows, on disk.
//!
//! Four files a project keeps in its `.metaform` directory and nothing above them: the scenario
//! state, the law theory, the evidence ledger, and the held-out predictions. They are one crate
//! because they share one reason to change -- adding a field to what a project knows edits all four
//! formats at once, and a schema version bump that reaches three of them is a bug with a name.
//!
//! None of the four depends on another. Each depends on the kernel for the objects it stores and, in
//! the case of predictions, on the protocol for the candidate it records. Nothing here decides
//! anything: a file is read, folded, and written back.
//!
//! `metaform` re-exports all four modules, so `metaform::state::State` is still the name a caller
//! writes.

pub mod state;

pub mod laws;

pub mod ledger;

pub mod predictions;
