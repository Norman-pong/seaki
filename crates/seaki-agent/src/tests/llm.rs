use crate::llm::*;
use crate::{AgentContext, AgentRuntime};

#[test]
fn mock_llm_returns_fixed_response() {
    let client = MockLlmClient::with_fixed_response("hello from mock".to_string());
    let request = LlmRequest {
        model: "mock".to_string(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "ping".to_string(),
            name: None,
        }],
        temperature: None,
        max_tokens: None,
    };
    let response = client.complete(request).unwrap();
    assert_eq!(response.content, "hello from mock");
}

#[test]
fn mock_llm_echoes_user_message() {
    let client = MockLlmClient::new();
    let request = LlmRequest {
        model: "mock".to_string(),
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: "You are a helper.".to_string(),
                name: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: "echo this".to_string(),
                name: None,
            },
        ],
        temperature: None,
        max_tokens: None,
    };
    let response = client.complete(request).unwrap();
    assert_eq!(response.content, "echo this");
}

#[test]
fn mock_llm_tracks_call_count() {
    let client = MockLlmClient::new();
    let request = LlmRequest {
        model: "mock".to_string(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "count me".to_string(),
            name: None,
        }],
        temperature: None,
        max_tokens: None,
    };
    assert_eq!(*client.call_count.lock().unwrap(), 0);
    client.complete(request.clone()).unwrap();
    assert_eq!(*client.call_count.lock().unwrap(), 1);
    client.complete(request).unwrap();
    assert_eq!(*client.call_count.lock().unwrap(), 2);
}

#[test]
fn open_ai_client_returns_error_without_server() {
    let config = crate::llm::OpenAiClientConfig {
        api_base: "http://127.0.0.1:1/v1".to_string(),
        api_key: "sk-test".to_string(),
        default_model: "gpt-4".to_string(),
        timeout_secs: 5,
    };
    let client = OpenAiClient::with_default_runtime(config);
    let request = LlmRequest {
        model: "gpt-4".to_string(),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: "hi".to_string(),
            name: None,
        }],
        temperature: None,
        max_tokens: None,
    };
    let err = client.complete(request).unwrap_err();
    // When no server is listening, we get a RequestFailed error (connection refused).
    assert!(matches!(err, LlmError::RequestFailed(_)));
}

#[test]
fn llm_request_serialize_roundtrip() {
    let original = LlmRequest {
        model: "test-model".to_string(),
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: "system prompt".to_string(),
                name: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: "user message".to_string(),
                name: None,
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(1024),
    };
    let json = serde_json::to_string(&original).unwrap();
    let roundtripped: LlmRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(original, roundtripped);
}

#[test]
fn agent_runtime_propose_pipeline() {
    let pipeline_json = r#"{"steps":[{"name":"ingest","command":"ingest_file"}]}"#;
    let client = MockLlmClient::with_fixed_response(pipeline_json.to_string());
    let runtime = AgentRuntime::new(Box::new(client));
    let context = AgentContext {
        workspace_id: "ws-1".to_string(),
        actor_id: "actor-1".to_string(),
        session_id: None,
    };
    let proposal = runtime.propose_pipeline("ingest a file", &context).unwrap();
    assert_eq!(proposal["steps"][0]["name"], "ingest");
    assert_eq!(proposal["steps"][0]["command"], "ingest_file");
}
