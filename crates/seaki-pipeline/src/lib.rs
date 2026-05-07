//! Pipeline Designer: compile user intent into validated, typed pipeline graphs.
//!
//! This crate sits above `seaki-pipe` and provides:
//! - `PipelineGraph` — a DAG-friendly representation of pipeline structure
//! - `Compiler` — type-checking, schema-hash validation, and policy/cost estimation
//! - `IntentParser` — a trait for translating natural-language intent into graphs
//! - `ManifestVersion` — command manifest versioning and compatibility checking

pub mod compiler;
pub mod cost;
pub mod graph;
pub mod intent;
pub mod policy;
pub mod version;

pub use compiler::{compile, compile_dag, CompileError, CompileResult};
pub use cost::{CostEstimate, CostEstimator};
pub use graph::{Edge, Node, NodeId, PipelineGraph};
pub use intent::{IntentParseError, IntentParser};
pub use policy::{PolicyEstimate, PolicyEstimator};
pub use version::{check_compatibility, ManifestVersion, VersionCompatibility};

#[cfg(test)]
pub use intent::MockIntentParser;

#[cfg(test)]
mod tests;
