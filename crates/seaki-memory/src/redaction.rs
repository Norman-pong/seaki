//! Redaction pipeline: secret scan, summary extraction.

use regex::Regex;
use std::sync::LazyLock;
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
#[must_use]
pub fn redact_and_summarize(transcript: &str) -> (String, RedactionStatus) {
    let (redacted, status) = redact_transcript(transcript);
    let summary = extract_summary(&redacted);
    (summary, status)
}

pub(crate) fn redact_transcript(text: &str) -> (String, RedactionStatus) {
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

static BEARER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(bearer\s+)[^,\n;]+").unwrap());

static SECRET_KV_RE: LazyLock<Regex> = LazyLock::new(|| {
    // S3: '&' is added to value delimiter so URL query params like
    // ?password=foo&api_key=bar are handled as separate matches.
    Regex::new(
        r#"(?i)((?:api[_-]?key|apikey|x[_-]?api[_-]?key|token|password|secret|auth[_-]?token|access[_-]?token|client[_-]?secret|private[_-]?key|aws[_-]?secret[_-]?access[_-]?key)"?\s*[:=]\s*)("[^"]*"|[^\s,;&"]+)"#,
    )
    .unwrap()
});

static AWS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)AKIA[0-9A-Z]{16}").unwrap());

static JWT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*").unwrap()
});

fn contains_secret_pattern(lower: &str) -> bool {
    lower.contains("authorization:")
        || BEARER_RE.is_match(lower)
        || SECRET_KV_RE.is_match(lower)
        || AWS_RE.is_match(lower)
        || JWT_RE.is_match(lower)
}

fn redact_line(line: &str) -> String {
    // bearer token
    let result = BEARER_RE.replace_all(line, "${1}[REDACTED]");
    if result != line {
        return result.to_string();
    }

    // key=value / key:value / JSON key:value / URL query param — 逐段处理，一行多个 secret 都脱敏
    let result = SECRET_KV_RE.replace_all(line, "${1}[REDACTED]");
    if result != line {
        return result.to_string();
    }

    // AWS credential
    let result = AWS_RE.replace_all(line, "[REDACTED]");
    if result != line {
        return result.to_string();
    }

    // JWT token
    let result = JWT_RE.replace_all(line, "[REDACTED]");
    if result != line {
        return result.to_string();
    }

    // 兜底整行脱敏
    "[REDACTED]".to_string()
}

pub(crate) fn extract_summary(redacted: &str) -> String {
    const MAX_SUMMARY_CHARS: usize = 200;
    let prefix: String = redacted.chars().take(MAX_SUMMARY_CHARS).collect();
    let suffix = if redacted.chars().count() > MAX_SUMMARY_CHARS {
        " ... [session summary]"
    } else {
        ""
    };
    format!("{prefix}{suffix}")
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
