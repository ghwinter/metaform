//! Concrete proposal components for metaform.
//!
//! The kernel and the operator protocol live in `metaform-runtime`. This crate holds the
//! components that implement the protocol: vector retrieval, structure mapping, label probing,
//! the relational-system reader, and the scenarios they share.
//!
//! The rule is the same as in the kernel: a component proposes, the kernel decides.

pub mod probe_labels;
pub mod scenarios;
pub mod sme_op;
pub mod systems;
pub mod vsa_op;
mod ilp_op;

pub use probe_labels::ProbeLabels;
pub use sme_op::SmeAnalogizer;
pub use systems::Systems;
pub use vsa_op::VsaRetriever;
pub use ilp_op::IlpInducer;
