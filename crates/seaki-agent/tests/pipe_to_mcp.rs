use std::collections::HashMap;

use seaki_agent::mcp::{McpError, PipeMcpServer, PipeToMcpAdapter};
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::run::CommandExecutor;

fn create_test_registry() -> CommandRegistry {
    CommandRegistry::builtin()
}

fn create_test_executors() -> HashMap<String, Box<dyn CommandExecutor>> {
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert(
        "wiki.search".to_string(),
        Box::new(seaki_pipe::run::WikiSearchExecutor),
    );
    executors.insert(
        "citation.resolve".to_string(),
        Box::new(seaki_pipe::run::CitationResolveExecutor),
    );
    executors.insert(
        "adr.summarize".to_string(),
        Box::new(seaki_pipe::run::AdrSummarizeExecutor),
    );
    executors.insert(
        "filter".to_string(),
        Box::new(seaki_pipe::run::FilterExecutor),
    );
    executors.insert("map".to_string(), Box::new(seaki_pipe::run::MapExecutor));
    executors.insert(
        "wiki.patch.propose".to_string(),
        Box::new(seaki_pipe::run::WikiPatchProposeExecutor),
    );
    executors
}

#[test]
fn adapter_converts_builtin_commands() {
    let registry = create_test_registry();
    let tools = PipeToMcpAdapter::from_registry(&registry);
    assert!(!tools.is_empty());
}

#[test]
fn adapter_filters_internal_commands() {
    let registry = create_test_registry();
    let tools = PipeToMcpAdapter::from_registry(&registry);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(!names.contains(&"filter"));
    assert!(!names.contains(&"map"));
}

#[test]
fn adapter_manifest_to_tool_schema_preserved() {
    let registry = create_test_registry();
    let manifest = registry.inspect("wiki.search").unwrap();
    let tool = PipeToMcpAdapter::manifest_to_tool(manifest);
    assert_eq!(tool.input_schema, manifest.input_schema);
}

#[test]
fn server_tools_list_returns_tools() {
    let registry = create_test_registry();
    let executors = create_test_executors();
    let mut server = PipeMcpServer::new(registry, executors);
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let response = server.handle_request(request).unwrap();
    assert!(response.contains("wiki.search"));
    assert!(response.contains("tools"));
}

#[test]
fn server_tools_call_executes_wiki_search() {
    let registry = create_test_registry();
    let executors = create_test_executors();
    let mut server = PipeMcpServer::new(registry, executors);
    let request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wiki.search","arguments":{"keyword":"test"}}}"#;
    let response = server.handle_request(request).unwrap();
    assert!(response.contains("simulated paragraph"));
}

#[test]
fn server_tools_call_unknown_tool() {
    let registry = create_test_registry();
    let executors = create_test_executors();
    let mut server = PipeMcpServer::new(registry, executors);
    let request = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"unknown.tool","arguments":{}}}"#;
    let result = server.handle_request(request);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::ToolNotFound(_)));
}

#[test]
fn server_handle_request_invalid_json() {
    let registry = create_test_registry();
    let executors = create_test_executors();
    let mut server = PipeMcpServer::new(registry, executors);
    let result = server.handle_request("not json");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::Protocol(_)));
}
