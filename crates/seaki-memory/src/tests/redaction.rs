use crate::redaction::{
    extract_summary, redact_transcript, RedactedSessionManifest, RedactionStatus,
};
use seaki_index::IndexScope;

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
fn redaction_preserves_multiple_secrets_on_one_line() {
    let input = "api_key=secret1 token=secret2 and done";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("secret1"));
    assert!(!redacted.contains("secret2"));
    assert!(redacted.contains("api_key=[REDACTED]"));
    assert!(redacted.contains("token=[REDACTED]"));
    assert!(redacted.contains("and done"));
}

#[test]
fn redaction_detects_api_key_variant_dash() {
    let input = "X-API-Key: abc123";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("abc123"));
    assert!(redacted.contains("X-API-Key: [REDACTED]"));
}

#[test]
fn redaction_detects_api_key_variant_underscore_prefix() {
    let input = "x_api_key=hidden";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("hidden"));
    assert!(redacted.contains("x_api_key=[REDACTED]"));
}

#[test]
fn redaction_detects_json_style_secret() {
    let input = r#"{"api_key":"shh","token":"hush"}"#;
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("shh"));
    assert!(!redacted.contains("hush"));
}

#[test]
fn manifest_defaults_ttl_to_30_days() {
    let manifest = RedactedSessionManifest::new(
        "s-1",
        "summary",
        IndexScope::new("ws", "ac"),
        "ref://original",
    );
    assert_eq!(manifest.ttl_seconds, 30 * 24 * 60 * 60);
    assert_eq!(manifest.session_id, "s-1");
    assert_eq!(manifest.original_transcript_ref, "ref://original");
}

#[test]
fn redaction_detects_auth_token() {
    let input = "auth_token=secret123";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(redacted.contains("auth_token=[REDACTED]"));
}

#[test]
fn redaction_detects_access_token() {
    let input = "access-token: secret123";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(redacted.contains("access-token: [REDACTED]"));
}

#[test]
fn redaction_detects_client_secret() {
    let input = "client_secret=shh";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(redacted.contains("client_secret=[REDACTED]"));
}

#[test]
fn redaction_detects_private_key() {
    let input = "private-key=hidden";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(redacted.contains("private-key=[REDACTED]"));
}

#[test]
fn redaction_detects_aws_secret_access_key() {
    let input = "aws_secret_access_key=AKIAIOSFODNN7EXAMPLE";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(redacted.contains("aws_secret_access_key=[REDACTED]"));
}

#[test]
fn redaction_detects_aws_access_key_id() {
    let input = "Key is AKIAIOSFODNN7EXAMPLE and done";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn redaction_detects_jwt_token() {
    let input = "token is eyJhbGciOiJIUzI1NiIs.eyJpc3MiOiJ0ZXN0In0.signature";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(!redacted.contains("eyJhbGciOiJIUzI1NiIs"));
    assert!(redacted.contains("[REDACTED]"));
}

// S3: URL query parameters with multiple secrets should all be redacted.
#[test]
fn redaction_url_query_params_multiple_secrets() {
    let input = "https://example.com/api?password=foo&api_key=bar&token=baz";
    let (redacted, status) = redact_transcript(input);
    assert_eq!(status, RedactionStatus::HasSecrets);
    assert!(
        !redacted.contains("foo"),
        "password value should be redacted: {redacted}"
    );
    assert!(
        !redacted.contains("bar"),
        "api_key value should be redacted: {redacted}"
    );
    assert!(
        !redacted.contains("baz"),
        "token value should be redacted: {redacted}"
    );
    assert!(redacted.contains("password="), "password key should remain");
    assert!(redacted.contains("api_key="), "api_key key should remain");
    assert!(redacted.contains("token="), "token key should remain");
}
