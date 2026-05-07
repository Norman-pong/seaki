pub mod dispatch;
pub mod llm;
pub mod session;
pub mod skill;
pub mod wal;

pub use dispatch::*;
pub use llm::*;
pub use session::*;
pub use skill::*;
pub use wal::*;

use serde_json::Value;

/// Context passed to the agent for pipeline proposal generation.
pub struct AgentContext {
    pub workspace_id: String,
    pub actor_id: String,
    pub session_id: Option<String>,
}

/// High-level agent runtime that orchestrates LLM calls and pipeline execution.
pub struct AgentRuntime {
    pub llm: Box<dyn LlmClient>,
}

impl AgentRuntime {
    pub fn new(llm: Box<dyn LlmClient>) -> Self {
        Self { llm }
    }

    /// Generate a pipeline proposal from user intent.
    pub fn propose_pipeline(
        &self,
        intent: &str,
        _context: &AgentContext,
    ) -> Result<Value, LlmError> {
        let request = LlmRequest {
            model: "mock".to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are a pipeline designer.".to_string(),
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: intent.to_string(),
                    name: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(2048),
        };
        let response = self.llm.complete(request)?;
        // Parse response.content as JSON pipeline proposal
        serde_json::from_str(&response.content).map_err(|e| LlmError::ParseFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests;
