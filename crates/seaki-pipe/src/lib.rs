pub mod approval_gate;
pub mod ast;
pub mod checkpoint;
pub mod compensate;
pub mod dag;
pub mod dry_run;
pub mod event;
pub mod executor;
pub mod registry;
pub mod run;
pub mod state_machine;

pub const SCHEMA_VERSION: u32 = 1;

pub use approval_gate::{
    ApprovalGate, ApprovalGateError, ApprovalRequestInput, InMemoryApprovalGate,
};
pub use ast::{
    compose, Cardinality, ComposeError, ComposedPipeline, ComposedStep, DagMergeStrategy,
    DagNodeKind, DagPipeline, DagStep, FailurePolicy, FrameType, InputBinding, PipelineAst,
    PipelineStep, TypedFrame,
};
pub use dry_run::{
    dry_run, DryRunEvent, DryRunResult, ErrorKind, FrameEnvelope, PatchProposalArtifact,
    PipelineError,
};
pub use event::{EventSink, EventSinkError, InMemoryEventSink, JsonlFileSink, PipelineEvent};
pub use registry::{
    CommandNotFound, CommandRegistry, PipeCommandManifest, PipeCommandSummary, RegistrationError,
    ResourceQuota, SideEffectFilter, SideEffectLevel,
};
pub use state_machine::{PipelineState, PipelineStateMachine, StateEvent, StateTransitionError};

#[cfg(test)]
mod tests;
