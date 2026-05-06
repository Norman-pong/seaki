use crate::intent::{IntentParseError, IntentParser, MockIntentParser};

#[test]
fn mock_parser_search_summarize() {
    let parser = MockIntentParser::new();
    let graph = parser.parse("search wiki and summarize results").unwrap();
    assert_eq!(graph.graph_id, "search_and_summarize");
    assert_eq!(graph.node_count(), 5); // entry + 3 commands + exit
}

#[test]
fn mock_parser_unrecognized() {
    let parser = MockIntentParser::new();
    let result = parser.parse("do something completely unknown");
    assert!(result.is_err());
    assert!(matches!(result, Err(IntentParseError::UnrecognizedIntent(_))));
}

#[test]
fn mock_parser_search_only() {
    let parser = MockIntentParser::new();
    let graph = parser.parse("search for architecture docs").unwrap();
    assert_eq!(graph.graph_id, "search");
}

#[test]
fn mock_parser_patch_propose() {
    let parser = MockIntentParser::new();
    let graph = parser.parse("propose a patch for the API").unwrap();
    assert_eq!(graph.graph_id, "patch_propose");
}
