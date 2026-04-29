//! Redaction pipeline: secret scan, summary extraction.

use std::time::{SystemTime, UNIX_EPOCH};

/// 脱敏后的会话摘要，不保存原始 transcript 内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedSessionManifest {
    pub session_id: String,
    pub summary: String,
    pub redacted_at: u64,
    pub ttl_seconds: u64,
    pub scope: seaki_index::IndexScope,
    pub original_transcript_ref: String,
}

impl RedactedSessionManifest {
    pub fn new(
        session_id: impl Into<String>,
        summary: impl Into<String>,
        scope: seaki_index::IndexScope,
        original_transcript_ref: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            summary: summary.into(),
            redacted_at: current_timestamp(),
            ttl_seconds: 30 * 24 * 60 * 60, // 默认 30 天
            scope,
            original_transcript_ref: original_transcript_ref.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionStatus {
    Clean,
    HasSecrets,
}

/// 最小脱敏 pipeline：正则（字符串模式）扫描常见 secret，提取摘要。
pub fn redact_and_summarize(transcript: &str) -> (String, RedactionStatus) {
    let (redacted, status) = redact_transcript(transcript);
    let summary = extract_summary(&redacted);
    (summary, status)
}

fn redact_transcript(text: &str) -> (String, RedactionStatus) {
    let mut has_secrets = false;
    let mut result = String::new();

    for line in text.lines() {
        let lower = line.to_lowercase();
        let redacted_line = if contains_secret_pattern(&lower) {
            has_secrets = true;
            redact_line(line)
        } else {
            line.to_string()
        };

        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&redacted_line);
    }

    let status = if has_secrets {
        RedactionStatus::HasSecrets
    } else {
        RedactionStatus::Clean
    };
    (result, status)
}

fn contains_secret_pattern(lower: &str) -> bool {
    lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("token=")
        || lower.contains("password=")
        || lower.contains("secret=")
        || lower.contains("bearer ")
        || lower.contains("authorization:")
}

fn redact_line(line: &str) -> String {
    let lower = line.to_lowercase();

    // bearer token 模式："bearer xxx"
    if let Some(pos) = lower.find("bearer ") {
        // 保留原始大小写中的 "Bearer" 前缀
        let prefix = &line[..pos];
        return format!("{}Bearer [REDACTED]", prefix);
    }

    // key=value 或 key: value 模式
    for sep in ['=', ':'] {
        if let Some(pos) = line.find(sep) {
            return format!("{}{}[REDACTED]", &line[..=pos], "");
        }
    }

    // 兜底整行脱敏
    "[REDACTED]".to_string()
}

fn extract_summary(redacted: &str) -> String {
    const MAX_SUMMARY_CHARS: usize = 200;
    let prefix: String = redacted.chars().take(MAX_SUMMARY_CHARS).collect();
    let suffix = if redacted.chars().count() > MAX_SUMMARY_CHARS {
        " ... [session summary]"
    } else {
        ""
    };
    format!("{}{}", prefix, suffix)
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_detects_api_key_and_masks_value() {
        let input = "user login api_key=secret123 and then done";
        let (redacted, status) = redact_transcript(input);
        assert_eq!(status, RedactionStatus::HasSecrets);
        assert!(!redacted.contains("secret123"));
        assert!(redacted.contains("api_key=[REDACTED]"));
    }

    #[test]
    fn redaction_detects_bearer_token() {
        let input = "Authorization: Bearer abc123.def456";
        let (redacted, status) = redact_transcript(input);
        assert_eq!(status, RedactionStatus::HasSecrets);
        assert!(!redacted.contains("abc123"));
        assert!(redacted.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn clean_text_passes_through() {
        let input = "user asked about rust ownership";
        let (redacted, status) = redact_transcript(input);
        assert_eq!(status, RedactionStatus::Clean);
        assert_eq!(redacted, input);
    }

    #[test]
    fn summary_is_truncated_to_200_chars_with_annotation() {
        let input = "a".repeat(250);
        let summary = extract_summary(&input);
        assert!(summary.contains("... [session summary]"));
        assert!(summary.len() <= 230); // 200 + suffix
    }

    #[test]
    fn manifest_defaults_ttl_to_30_days() {
        let manifest = RedactedSessionManifest::new(
            "s-1",
            "summary",
            seaki_index::IndexScope::new("ws", "ac"),
            "ref://original",
        );
        assert_eq!(manifest.ttl_seconds, 30 * 24 * 60 * 60);
        assert_eq!(manifest.session_id, "s-1");
        assert_eq!(manifest.original_transcript_ref, "ref://original");
    }
}
