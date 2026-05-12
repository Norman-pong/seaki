//! LLM-driven citation-backed answer composer.
//!
//! Given search results and a user query, generates a natural-language answer
//! via LLM with embedded `[1]`, `[2]` ... citation markers.

use crate::llm::{LlmClient, LlmMessage, LlmRequest, MessageRole};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single search-result entry used as context for the LLM.
pub struct SearchContextItem {
    pub title: String,
    pub snippet: String,
    pub citation_id: String,
    pub source_id: String,
}

/// Request sent to [`AnswerComposer::compose`].
pub struct ComposeRequest {
    pub query: String,
    pub search_results: Vec<SearchContextItem>,
    pub workspace_id: String,
}

/// Response (mirrors `seaki-core`'s `AnswerDTO` structure).
pub struct ComposeResult {
    pub answer_id: String,
    pub text: String,
    pub citation_refs: Vec<CitationRef>,
    /// `"composed"` | `"degraded"` | `"fallback"`
    pub status: String,
}

/// A citation reference extracted from the LLM output.
pub struct CitationRef {
    pub citation_id: String,
    pub source_id: String,
    /// The 1-based index appearing as `[N]` in the answer text.
    pub index: usize,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise during answer composition.
#[derive(Debug)]
pub enum ComposeError {
    /// The underlying LLM call failed.
    LlmFailed(String),
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::LlmFailed(msg) => write!(f, "LLM call failed: {msg}"),
        }
    }
}

impl std::error::Error for ComposeError {}

// ---------------------------------------------------------------------------
// AnswerComposer
// ---------------------------------------------------------------------------

/// LLM-driven answer composer that replaces simple snippet concatenation.
pub struct AnswerComposer {
    llm: Box<dyn LlmClient>,
}

impl AnswerComposer {
    /// Create a new composer backed by the given LLM client.
    pub fn new(llm: Box<dyn LlmClient>) -> Self {
        Self { llm }
    }

    /// Compose a citation-backed answer for the given query.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError::LlmFailed`] when the underlying LLM call fails.
    pub fn compose(&self, request: ComposeRequest) -> Result<ComposeResult, ComposeError> {
        // 1. Empty results → fallback immediately.
        if request.search_results.is_empty() {
            return Ok(ComposeResult {
                answer_id: generate_answer_id(&request.workspace_id, &request.query),
                text: String::new(),
                citation_refs: Vec::new(),
                status: "fallback".to_string(),
            });
        }

        // 2. Build the system prompt with formatted search results.
        let formatted_results = format_search_results(&request.search_results);
        let system_content = format!(
            "你是一个知识助手。根据以下搜索结果回答用户问题。\n\n\
             要求：\n\
             1. 在回答中使用 [1], [2] 等标记引用来源\n\
             2. 只使用提供的搜索结果中的信息，不要编造\n\
             3. 如果搜索结果不足以回答问题，明确说明\n\n\
             搜索结果：\n\
             {formatted_results}"
        );

        // 3. Build the LLM request.
        let llm_request = LlmRequest {
            model: String::new(), // will use the client's default model
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: system_content,
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: request.query.clone(),
                    name: None,
                },
            ],
            temperature: Some(0.3),
            max_tokens: None,
        };

        // 4. Call the LLM.
        let response = self
            .llm
            .complete(llm_request)
            .map_err(|e| ComposeError::LlmFailed(e.to_string()))?;

        let text = response.content;

        // 5. Extract citation markers from the output.
        let citation_refs = extract_citation_refs(&text, &request.search_results);

        // 6. Determine status.
        let status = if citation_refs.is_empty() {
            "degraded"
        } else {
            "composed"
        };

        Ok(ComposeResult {
            answer_id: generate_answer_id(&request.workspace_id, &request.query),
            text,
            citation_refs,
            status: status.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn generate_answer_id(workspace_id: &str, query: &str) -> String {
    format!("answer-{workspace_id}-{}", crate::safe_truncate(query, 32))
}

fn format_search_results(results: &[SearchContextItem]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, item)| format!("[{}] {}: {}", i + 1, item.title, item.snippet))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract `[N]` markers from `text`, deduplicate, sort, and map back to the
/// corresponding [`SearchContextItem`]. Out-of-range indices are silently
/// ignored.
fn extract_citation_refs(text: &str, results: &[SearchContextItem]) -> Vec<CitationRef> {
    let mut indices: Vec<usize> = Vec::new();

    // Simple parsing: scan for '[' followed by digits followed by ']'.
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            // Try to read a number followed by ']'.
            let num_start = i + 1;
            let mut num_end = num_start;
            while num_end < chars.len() && chars[num_end].is_ascii_digit() {
                num_end += 1;
            }
            if num_end > num_start && num_end < chars.len() && chars[num_end] == ']' {
                let num_str: String = chars[num_start..num_end].iter().collect();
                if let Ok(n) = num_str.parse::<usize>() {
                    if n >= 1 && !indices.contains(&n) {
                        indices.push(n);
                    }
                }
            }
        }
        i += 1;
    }

    indices.sort_unstable();

    indices
        .into_iter()
        .filter_map(|n| {
            let idx = n - 1; // 1-based to 0-based
            results.get(idx).map(|item| CitationRef {
                citation_id: item.citation_id.clone(),
                source_id: item.source_id.clone(),
                index: n,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockLlmClient;

    /// Helper to build a search context item.
    fn make_item(citation_id: &str, source_id: &str, title: &str, snippet: &str) -> SearchContextItem {
        SearchContextItem {
            title: title.to_string(),
            snippet: snippet.to_string(),
            citation_id: citation_id.to_string(),
            source_id: source_id.to_string(),
        }
    }

    /// Helper to build a compose request.
    fn make_request(query: &str, results: Vec<SearchContextItem>) -> ComposeRequest {
        ComposeRequest {
            query: query.to_string(),
            search_results: results,
            workspace_id: "ws-test".to_string(),
        }
    }

    #[test]
    fn compose_with_mock_llm_returns_answer() {
        let mock = MockLlmClient::with_fixed_response(
            "根据搜索结果，Rust 是一种系统编程语言 [1]。它强调安全和并发 [2]。".to_string(),
        );
        let composer = AnswerComposer::new(Box::new(mock));

        let request = make_request(
            "什么是 Rust？",
            vec![
                make_item("cit-1", "src-1", "Rust 简介", "Rust 是一种系统编程语言"),
                make_item("cit-2", "src-2", "Rust 特性", "Rust 强调安全和并发"),
            ],
        );

        let result = composer.compose(request).unwrap();
        assert_eq!(result.status, "composed");
        assert!(!result.text.is_empty());
        assert_eq!(result.citation_refs.len(), 2);

        assert_eq!(result.citation_refs[0].citation_id, "cit-1");
        assert_eq!(result.citation_refs[0].source_id, "src-1");
        assert_eq!(result.citation_refs[0].index, 1);

        assert_eq!(result.citation_refs[1].citation_id, "cit-2");
        assert_eq!(result.citation_refs[1].source_id, "src-2");
        assert_eq!(result.citation_refs[1].index, 2);
    }

    #[test]
    fn compose_with_empty_results_returns_fallback() {
        let mock = MockLlmClient::new();
        let composer = AnswerComposer::new(Box::new(mock));

        let request = make_request("什么是 Rust？", vec![]);
        let result = composer.compose(request).unwrap();

        assert_eq!(result.status, "fallback");
        assert!(result.text.is_empty());
        assert!(result.citation_refs.is_empty());
    }

    #[test]
    fn compose_extracts_citation_markers() {
        let mock = MockLlmClient::with_fixed_response(
            "第一点 [2]，第二点 [1]，重复 [2] 不应产生重复引用。".to_string(),
        );
        let composer = AnswerComposer::new(Box::new(mock));

        let request = make_request(
            "测试",
            vec![
                make_item("cit-a", "src-a", "A", "snippet a"),
                make_item("cit-b", "src-b", "B", "snippet b"),
                make_item("cit-c", "src-c", "C", "snippet c"),
            ],
        );

        let result = composer.compose(request).unwrap();
        assert_eq!(result.status, "composed");
        // Should have 2 unique refs: [1] and [2], sorted.
        assert_eq!(result.citation_refs.len(), 2);
        assert_eq!(result.citation_refs[0].index, 1);
        assert_eq!(result.citation_refs[0].citation_id, "cit-a");
        assert_eq!(result.citation_refs[1].index, 2);
        assert_eq!(result.citation_refs[1].citation_id, "cit-b");
    }

    #[test]
    fn compose_handles_no_citation_markers() {
        let mock = MockLlmClient::with_fixed_response(
            "根据我所知，这是一个普通的回答，没有任何引用标记。".to_string(),
        );
        let composer = AnswerComposer::new(Box::new(mock));

        let request = make_request(
            "测试",
            vec![make_item("cit-1", "src-1", "标题", "内容")],
        );

        let result = composer.compose(request).unwrap();
        // No citation markers → degraded
        assert_eq!(result.status, "degraded");
        assert!(!result.text.is_empty());
        assert!(result.citation_refs.is_empty());
    }

    #[test]
    fn compose_handles_out_of_range_citation() {
        let mock = MockLlmClient::with_fixed_response(
            "这个引用 [99] 超出范围，这个 [1] 是有效的。".to_string(),
        );
        let composer = AnswerComposer::new(Box::new(mock));

        let request = make_request(
            "测试",
            vec![make_item("cit-1", "src-1", "标题", "内容")],
        );

        let result = composer.compose(request).unwrap();
        assert_eq!(result.status, "composed");
        // [99] is out of range, only [1] should be kept.
        assert_eq!(result.citation_refs.len(), 1);
        assert_eq!(result.citation_refs[0].index, 1);
        assert_eq!(result.citation_refs[0].citation_id, "cit-1");
    }
}
