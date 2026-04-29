pub mod ast;
pub mod dry_run;
pub mod registry;

pub const SCHEMA_VERSION: u32 = 1;

pub use ast::{
    compose, Cardinality, ComposeError, ComposedPipeline, ComposedStep, FailurePolicy, FrameType,
    InputBinding, PipelineAst, PipelineStep, TypedFrame,
};
pub use dry_run::{
    dry_run, DryRunEvent, DryRunResult, ErrorKind, FrameEnvelope, PatchProposalArtifact,
    PipelineError,
};
pub use registry::{
    CommandNotFound, CommandRegistry, PipeCommandManifest, PipeCommandSummary, RegistrationError,
    ResourceQuota, SideEffectFilter, SideEffectLevel,
};
