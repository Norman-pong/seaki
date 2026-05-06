//! Intent parser: translate natural-language intent into a `PipelineGraph`.
//!
//! For M2-P01, this module provides a trait and a mock implementation.
//! Real LLM-based parsing will be wired up in M2-A01 (Agent Runtime).

use crate::graph::{GraphError, Node, NodeId, PipelineGraph};

/// Errors that can occur during intent parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentParseError {
    UnrecognizedIntent(String),
    AmbiguousIntent(String),
    UnsupportedCommand(String),
    Graph(GraphError),
}

impl std::fmt::Display for IntentParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnrecognizedIntent(s) => write!(f, "unrecognized intent: {s}"),
            Self::AmbiguousIntent(s) => write!(f, "ambiguous intent: {s}"),
            Self::UnsupportedCommand(s) => write!(f, "unsupported command: {s}"),
            Self::Graph(e) => write!(f, "graph error: {e}"),
        }
    }
}

impl std::error::Error for IntentParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graph(e) => Some(e),
            _ => None,
        }
    }
}

impl From<GraphError> for IntentParseError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

/// Trait for intent parsers.
pub trait IntentParser {
    /// Parse a natural-language intent into a `PipelineGraph`.
    ///
    /// # Errors
    /// Returns `IntentParseError` if the intent cannot be parsed.
    fn parse(&self, intent: &str) -> Result<PipelineGraph, IntentParseError>;
}

/// A mock intent parser for testing and development.
///
/// Recognizes a small set of hard-coded intents and produces corresponding graphs.
#[derive(Debug, Clone, Default)]
pub struct MockIntentParser;

impl MockIntentParser {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl IntentParser for MockIntentParser {
    fn parse(&self, intent: &str) -> Result<PipelineGraph, IntentParseError> {
        let intent_lower = intent.to_ascii_lowercase();

        if intent_lower.contains("search") && intent_lower.contains("summarize") {
            build_search_summarize_graph()
        } else if intent_lower.contains("search") {
            build_search_graph()
        } else if intent_lower.contains("summarize") {
            build_summarize_graph()
        } else if intent_lower.contains("patch") || intent_lower.contains("propose") {
            build_patch_propose_graph()
        } else {
            Err(IntentParseError::UnrecognizedIntent(intent.to_string()))
        }
    }
}

/// Build a linear pipeline graph from a list of commands.
///
/// The graph structure is: Entry -> Command[0] -> Command[1] -> ... -> Exit.
fn build_linear_graph(
    graph_id: &str,
    commands: &[(NodeId, String, serde_json::Value)],
) -> Result<PipelineGraph, IntentParseError> {
    let mut graph = PipelineGraph::new(graph_id);

    graph.add_node(Node::Entry {
        node_id: NodeId::from("entry"),
    })?;

    for (node_id, command_id, args) in commands {
        graph.add_node(Node::Command {
            node_id: node_id.clone(),
            command_id: command_id.clone(),
            args: args.clone(),
        })?;
    }

    graph.add_node(Node::Exit {
        node_id: NodeId::from("exit"),
    })?;

    graph.set_entry(NodeId::from("entry"))?;
    graph.add_exit(NodeId::from("exit"))?;

    // Wire edges: entry -> cmd[0] -> cmd[1] -> ... -> exit
    let mut prev = NodeId::from("entry");
    for (node_id, _, _) in commands {
        graph.add_edge(prev, node_id.clone())?;
        prev = node_id.clone();
    }
    graph.add_edge(prev, NodeId::from("exit"))?;

    Ok(graph)
}

fn build_search_graph() -> Result<PipelineGraph, IntentParseError> {
    build_linear_graph(
        "search",
        &[(
            NodeId::from("search"),
            "wiki.search".to_string(),
            serde_json::json!({"keyword": "__intent_keyword__"}),
        )],
    )
}

fn build_summarize_graph() -> Result<PipelineGraph, IntentParseError> {
    build_linear_graph(
        "summarize",
        &[(
            NodeId::from("summarize"),
            "adr.summarize".to_string(),
            serde_json::json!({}),
        )],
    )
}

fn build_search_summarize_graph() -> Result<PipelineGraph, IntentParseError> {
    build_linear_graph(
        "search_and_summarize",
        &[
            (
                NodeId::from("search"),
                "wiki.search".to_string(),
                serde_json::json!({"keyword": "__intent_keyword__"}),
            ),
            (
                NodeId::from("resolve"),
                "citation.resolve".to_string(),
                serde_json::json!({}),
            ),
            (
                NodeId::from("summarize"),
                "adr.summarize".to_string(),
                serde_json::json!({}),
            ),
        ],
    )
}

fn build_patch_propose_graph() -> Result<PipelineGraph, IntentParseError> {
    build_linear_graph(
        "patch_propose",
        &[
            (
                NodeId::from("search"),
                "wiki.search".to_string(),
                serde_json::json!({"keyword": "__intent_keyword__"}),
            ),
            (
                NodeId::from("resolve"),
                "citation.resolve".to_string(),
                serde_json::json!({}),
            ),
            (
                NodeId::from("propose"),
                "wiki.patch.propose".to_string(),
                serde_json::json!({}),
            ),
        ],
    )
}
