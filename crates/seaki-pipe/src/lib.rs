pub mod ast;
pub mod checkpoint;
pub mod compensate;
pub mod dag;
pub mod dry_run;
pub mod executor;
pub mod registry;
pub mod run;

pub const SCHEMA_VERSION: u32 = 1;

pub use ast::{
    compose, Cardinality, ComposeError, ComposedPipeline, ComposedStep, DagMergeStrategy,
    DagNodeKind, DagPipeline, DagStep, FailurePolicy, FrameType, InputBinding, PipelineAst,
    PipelineStep, TypedFrame,
};
pub use dry_run::{
    dry_run, DryRunEvent, DryRunResult, ErrorKind, FrameEnvelope, PatchProposalArtifact,
    PipelineError,
};
pub use registry::{
    CommandNotFound, CommandRegistry, PipeCommandManifest, PipeCommandSummary, RegistrationError,
    ResourceQuota, SideEffectFilter, SideEffectLevel,
};

#[cfg(test)]
mod tests;
