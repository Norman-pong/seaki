use crate::graph::{GraphError, Node, NodeId, PipelineGraph};

#[test]
fn graph_linear_chain_validates() {
    let mut graph = PipelineGraph::new("test_linear");
    graph
        .add_node(Node::Entry {
            node_id: NodeId::from("entry"),
        })
        .unwrap();
    graph
        .add_node(Node::Command {
            node_id: NodeId::from("search"),
            command_id: "wiki.search".to_string(),
            args: serde_json::json!({}),
        })
        .unwrap();
    graph
        .add_node(Node::Exit {
            node_id: NodeId::from("exit"),
        })
        .unwrap();
    graph.set_entry(NodeId::from("entry")).unwrap();
    graph.add_exit(NodeId::from("exit")).unwrap();
    graph
        .add_edge(NodeId::from("entry"), NodeId::from("search"))
        .unwrap();
    graph
        .add_edge(NodeId::from("search"), NodeId::from("exit"))
        .unwrap();

    assert!(graph.validate().is_ok());
}

#[test]
fn graph_detects_cycle() {
    let mut graph = PipelineGraph::new("test_cycle");
    graph
        .add_node(Node::Entry {
            node_id: NodeId::from("entry"),
        })
        .unwrap();
    graph
        .add_node(Node::Command {
            node_id: NodeId::from("a"),
            command_id: "wiki.search".to_string(),
            args: serde_json::json!({}),
        })
        .unwrap();
    graph
        .add_node(Node::Command {
            node_id: NodeId::from("b"),
            command_id: "citation.resolve".to_string(),
            args: serde_json::json!({}),
        })
        .unwrap();
    graph
        .add_node(Node::Exit {
            node_id: NodeId::from("exit"),
        })
        .unwrap();
    graph.set_entry(NodeId::from("entry")).unwrap();
    graph.add_exit(NodeId::from("exit")).unwrap();
    graph.add_edge(NodeId::from("entry"), NodeId::from("a")).unwrap();
    graph.add_edge(NodeId::from("a"), NodeId::from("b")).unwrap();
    graph.add_edge(NodeId::from("b"), NodeId::from("a")).unwrap(); // cycle

    let result = graph.validate();
    assert!(matches!(result, Err(GraphError::CycleDetected)));
}

#[test]
fn graph_rejects_duplicate_node_id() {
    let mut graph = PipelineGraph::new("test_dup");
    graph
        .add_node(Node::Entry {
            node_id: NodeId::from("entry"),
        })
        .unwrap();
    let result = graph.add_node(Node::Entry {
        node_id: NodeId::from("entry"),
    });
    assert!(matches!(result, Err(GraphError::DuplicateNodeId(_))));
}

#[test]
fn graph_rejects_dangling_edge() {
    let mut graph = PipelineGraph::new("test_dangling");
    graph
        .add_node(Node::Entry {
            node_id: NodeId::from("entry"),
        })
        .unwrap();
    let result = graph.add_edge(NodeId::from("entry"), NodeId::from("ghost"));
    assert!(matches!(result, Err(GraphError::NodeNotFound(_))));
}

#[test]
fn graph_to_linear_ast_produces_steps() {
    let mut graph = PipelineGraph::new("test_linear_ast");
    graph
        .add_node(Node::Entry {
            node_id: NodeId::from("entry"),
        })
        .unwrap();
    graph
        .add_node(Node::Command {
            node_id: NodeId::from("search"),
            command_id: "wiki.search".to_string(),
            args: serde_json::json!({"keyword": "test"}),
        })
        .unwrap();
    graph
        .add_node(Node::Command {
            node_id: NodeId::from("resolve"),
            command_id: "citation.resolve".to_string(),
            args: serde_json::json!({}),
        })
        .unwrap();
    graph
        .add_node(Node::Exit {
            node_id: NodeId::from("exit"),
        })
        .unwrap();
    graph.set_entry(NodeId::from("entry")).unwrap();
    graph.add_exit(NodeId::from("exit")).unwrap();
    graph
        .add_edge(NodeId::from("entry"), NodeId::from("search"))
        .unwrap();
    graph
        .add_edge(NodeId::from("search"), NodeId::from("resolve"))
        .unwrap();
    graph
        .add_edge(NodeId::from("resolve"), NodeId::from("exit"))
        .unwrap();

    let ast = graph.to_linear_ast().unwrap();
    assert_eq!(ast.steps.len(), 2);
    assert_eq!(ast.steps[0].command_id, "wiki.search");
    assert_eq!(ast.steps[1].command_id, "citation.resolve");
    // First step should use Constant binding, second should use PreviousStep.
    assert!(matches!(
        ast.steps[0].input_binding,
        seaki_pipe::InputBinding::Constant(_)
    ));
    assert!(matches!(
        ast.steps[1].input_binding,
        seaki_pipe::InputBinding::PreviousStep
    ));
}
