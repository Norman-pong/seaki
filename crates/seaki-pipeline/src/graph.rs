//! PipelineGraph: a DAG representation of pipeline structure.
//!
//! Unlike `PipelineAst` (linear steps), `PipelineGraph` supports branching
//! (`tee`/`branch`) and joining (`join`) as first-class concepts.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Opaque identifier for a node in the pipeline graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A node in the pipeline graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    /// A command invocation.
    Command {
        node_id: NodeId,
        command_id: String,
        args: serde_json::Value,
    },
    /// Split one stream into multiple branches.
    Tee {
        node_id: NodeId,
        branches: Vec<NodeId>,
    },
    /// Conditionally route frames to one of several branches.
    Branch {
        node_id: NodeId,
        condition: BranchCondition,
        branches: Vec<(NodeId, serde_json::Value)>, // (target, predicate value)
    },
    /// Merge multiple branches back into one stream.
    Join {
        node_id: NodeId,
        sources: Vec<NodeId>,
        merge_strategy: MergeStrategy,
    },
    /// Entry point of the graph.
    Entry { node_id: NodeId },
    /// Exit point of the graph.
    Exit { node_id: NodeId },
}

impl Node {
    #[must_use]
    pub fn node_id(&self) -> &NodeId {
        match self {
            Self::Command { node_id, .. }
            | Self::Tee { node_id, .. }
            | Self::Branch { node_id, .. }
            | Self::Join { node_id, .. }
            | Self::Entry { node_id }
            | Self::Exit { node_id } => node_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchCondition {
    FrameType,
    Predicate { field: String, op: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    Concat,
    Interleave,
    FirstNonEmpty,
}

/// A directed edge in the pipeline graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub label: Option<String>,
}

/// The pipeline graph: a directed acyclic graph of pipeline nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineGraph {
    pub graph_id: String,
    pub edges: Vec<Edge>,
    nodes: HashMap<NodeId, Node>,
    entry: NodeId,
    exits: Vec<NodeId>,
}

impl PipelineGraph {
    /// Lookup a node by its id.
    #[must_use]
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Iterate over all node ids in the graph.
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.nodes.keys()
    }

    /// Number of nodes in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return the entry node id.
    #[must_use]
    pub fn entry_id(&self) -> &NodeId {
        &self.entry
    }

    /// Return the exit node ids.
    #[must_use]
    pub fn exit_ids(&self) -> &[NodeId] {
        &self.exits
    }
}

/// Errors that can occur when constructing or validating a `PipelineGraph`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNodeId(NodeId),
    NodeNotFound(NodeId),
    CycleDetected,
    NotLinearGraph { reason: String },
    MissingEntry,
    MissingExit,
    DanglingEdge { from: NodeId, to: NodeId },
    MultipleEntries,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNodeId(id) => write!(f, "duplicate node id: {id}"),
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::CycleDetected => write!(f, "cycle detected in pipeline graph"),
            Self::NotLinearGraph { reason } => {
                write!(f, "graph is not a linear chain: {reason}")
            }
            Self::MissingEntry => write!(f, "pipeline graph missing entry node"),
            Self::MissingExit => write!(f, "pipeline graph missing exit node"),
            Self::DanglingEdge { from, to } => {
                write!(f, "dangling edge from {from} to {to}")
            }
            Self::MultipleEntries => write!(f, "pipeline graph has multiple entry nodes"),
        }
    }
}

impl std::error::Error for GraphError {}

impl PipelineGraph {
    /// Create a new empty pipeline graph.
    #[must_use]
    pub fn new(graph_id: impl Into<String>) -> Self {
        Self {
            graph_id: graph_id.into(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            entry: NodeId::from("__dummy"),
            exits: Vec::new(),
        }
    }

    /// Add a node to the graph.
    ///
    /// # Errors
    /// Returns `GraphError::DuplicateNodeId` if the node ID already exists.
    pub fn add_node(&mut self, node: Node) -> Result<(), GraphError> {
        let id = node.node_id().clone();
        if self.nodes.contains_key(&id) {
            return Err(GraphError::DuplicateNodeId(id));
        }
        self.nodes.insert(id, node);
        Ok(())
    }

    /// Add a directed edge between two nodes.
    ///
    /// # Errors
    /// Returns `GraphError::NodeNotFound` if either endpoint does not exist.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) -> Result<(), GraphError> {
        if !self.nodes.contains_key(&from) {
            return Err(GraphError::NodeNotFound(from));
        }
        if !self.nodes.contains_key(&to) {
            return Err(GraphError::NodeNotFound(to));
        }
        self.edges.push(Edge {
            from,
            to,
            label: None,
        });
        Ok(())
    }

    /// Set the entry node of the graph.
    ///
    /// # Errors
    /// Returns `GraphError::NodeNotFound` if the node does not exist.
    pub fn set_entry(&mut self, entry: NodeId) -> Result<(), GraphError> {
        if !self.nodes.contains_key(&entry) {
            return Err(GraphError::NodeNotFound(entry));
        }
        self.entry = entry;
        Ok(())
    }

    /// Add an exit node to the graph.
    ///
    /// # Errors
    /// Returns `GraphError::NodeNotFound` if the node does not exist.
    pub fn add_exit(&mut self, exit: NodeId) -> Result<(), GraphError> {
        if !self.nodes.contains_key(&exit) {
            return Err(GraphError::NodeNotFound(exit));
        }
        self.exits.push(exit);
        Ok(())
    }

    /// Validate the graph: no cycles, all edges resolved, exactly one entry, at least one exit.
    ///
    /// # Errors
    /// Returns `GraphError` if validation fails.
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.nodes.is_empty() {
            return Err(GraphError::MissingEntry);
        }

        // Check entry exists and is an Entry node.
        let entry_node = self
            .nodes
            .get(&self.entry)
            .ok_or(GraphError::MissingEntry)?;
        if !matches!(entry_node, Node::Entry { .. }) {
            return Err(GraphError::MissingEntry);
        }

        // Check at least one exit.
        if self.exits.is_empty() {
            return Err(GraphError::MissingExit);
        }
        for exit in &self.exits {
            let exit_node = self.nodes.get(exit).ok_or(GraphError::MissingExit)?;
            if !matches!(exit_node, Node::Exit { .. }) {
                return Err(GraphError::MissingExit);
            }
        }

        // Check all edges reference existing nodes.
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) || !self.nodes.contains_key(&edge.to) {
                return Err(GraphError::DanglingEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
            }
        }

        // Detect cycles via DFS.
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            graph.entry(&edge.from.0).or_default().push(&edge.to.0);
        }

        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        for node_id in self.nodes.keys() {
            if !visited.contains(node_id.0.as_str()) {
                Self::dfs(node_id.0.as_str(), &graph, &mut visited, &mut stack)?;
            }
        }

        Ok(())
    }

    /// Linearise the graph into a `PipelineAst` when the graph is a simple linear chain.
    ///
    /// # Errors
    /// Returns `GraphError` if the graph is not a linear chain.
    pub fn to_linear_ast(&self) -> Result<seaki_pipe::PipelineAst, GraphError> {
        // Build adjacency list.
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            outgoing.entry(&edge.from.0).or_default().push(&edge.to.0);
            incoming.entry(&edge.to.0).or_default().push(&edge.from.0);
        }

        // Walk from entry to exit, checking linearity.
        let mut steps: Vec<seaki_pipe::PipelineStep> = Vec::new();
        let mut current = self.entry.0.as_str();
        let exit_node = self.exits.first().ok_or(GraphError::MissingExit)?;
        let mut is_first_command = true;

        while current != exit_node.0.as_str() {
            let node = self
                .nodes
                .get(&NodeId::from(current))
                .ok_or_else(|| GraphError::NodeNotFound(NodeId::from(current)))?;

            match node {
                Node::Command {
                    node_id,
                    command_id,
                    args,
                } => {
                    let input_binding = if is_first_command {
                        seaki_pipe::InputBinding::Constant(args.clone())
                    } else {
                        seaki_pipe::InputBinding::PreviousStep
                    };
                    steps.push(seaki_pipe::PipelineStep {
                        step_id: node_id.0.clone(),
                        command_id: command_id.clone(),
                        input_binding,
                        args: serde_json::json!({}),
                        failure_policy: seaki_pipe::FailurePolicy::FailFast,
                    });
                    is_first_command = false;
                }
                Node::Tee { .. } | Node::Branch { .. } | Node::Join { .. } => {
                    return Err(GraphError::NotLinearGraph {
                        reason: "graph contains tee/branch/join nodes".to_string(),
                    });
                }
                Node::Entry { .. } | Node::Exit { .. } => {}
            }

            let next_nodes = outgoing
                .get(current)
                .ok_or_else(|| GraphError::NotLinearGraph {
                    reason: "dead end detected".to_string(),
                })?;
            if next_nodes.len() != 1 {
                return Err(GraphError::NotLinearGraph {
                    reason: "branch or merge detected".to_string(),
                });
            }
            current = next_nodes[0];
        }

        Ok(seaki_pipe::PipelineAst {
            pipeline_id: self.graph_id.clone(),
            steps,
        })
    }

    fn dfs<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        stack: &mut HashSet<&'a str>,
    ) -> Result<(), GraphError> {
        visited.insert(node);
        stack.insert(node);

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    Self::dfs(neighbor, graph, visited, stack)?;
                } else if stack.contains(neighbor) {
                    return Err(GraphError::CycleDetected);
                }
            }
        }

        stack.remove(node);
        Ok(())
    }
}
