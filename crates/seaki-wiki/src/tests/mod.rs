use super::*;
use tempfile::tempdir;

fn ingest_source(cas: &RawCas, input: SourceInput) -> WikiResult<SourceIngestResult> {
    ingest_source_with_config(cas, input, ParserConfig::default())
}

fn ingest_source_with_config(
    cas: &RawCas,
    input: SourceInput,
    config: ParserConfig,
) -> WikiResult<SourceIngestResult> {
    let raw_blob = cas.append(&input.bytes)?;
    Ok(ingest_committed_source(raw_blob, input, config))
}

#[test]
fn wiki_declares_append_only_source_layer() {
    assert_eq!(RAW_SOURCE_STORAGE, "append-only-content-addressed");
    assert!(SourceIngestState::Indexed.is_terminal());
    assert!(!SourceIngestState::ParseRunning.is_terminal());
}

#[test]
fn markdown_ingest_commits_raw_and_parses_frames() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let markdown = "# Title\n\nHello [link](https://example.com).\n";
    let result = ingest_source(&cas, markdown_input(markdown)).expect("markdown ingest");

    assert_eq!(result.manifest.parse_status, ImportStage::Parsed);
    assert_eq!(
        result.manifest.state_history,
        [
            ImportStage::RawCommitted,
            ImportStage::ParseRunning,
            ImportStage::Parsed
        ]
    );
    assert_eq!(result.artifact.parser_version, MARKDOWN_PARSER_VERSION);
    assert_eq!(result.artifact.frames.len(), 2);

    let frame = &result.artifact.frames[1];
    assert_eq!(frame.source_id, result.manifest.source_id);
    assert_eq!(frame.source_hash, result.artifact.source_hash);
    assert_eq!(frame.parser_version, MARKDOWN_PARSER_VERSION);
    assert_eq!(frame.line_range, LineRange { start: 3, end: 3 });
    assert_eq!(frame.byte_range, ByteRange { start: 9, end: 43 });
    assert_eq!(
        frame.text.as_bytes(),
        &markdown.as_bytes()[frame.byte_range.start..frame.byte_range.end]
    );
    assert_eq!(frame.mime_sniff.sniffed, "text/markdown");
    assert_eq!(frame.text_hash, sha256_hex(frame.text.as_bytes()));
    assert_eq!(frame.trust_level, TrustLevel::Untrusted);
    assert_eq!(frame.taint, Taint::UntrustedContent);
    assert_eq!(frame.schema_hash, PARSED_FRAME_SCHEMA_HASH);
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::UntrustedContent));
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::ExternalReference));
}

#[test]
fn raw_cas_is_append_only_and_workspace_keyed() {
    let temp = tempdir().expect("tempdir");
    let content = b"same source bytes";
    let cas_a = RawCas::new(temp.path().join("a"), b"workspace-a").expect("cas a");
    let cas_b = RawCas::new(temp.path().join("b"), b"workspace-b").expect("cas b");

    let first = cas_a.append(content).expect("first append");
    let second = cas_a.append(content).expect("second append");
    let other_workspace = cas_b.append(content).expect("other append");

    assert!(first.newly_written);
    assert!(!second.newly_written);
    assert_eq!(first.raw_key, second.raw_key);
    assert_ne!(first.raw_key, first.content_hash);
    assert_eq!(first.path, second.path);
    assert_ne!(first.raw_key, other_workspace.raw_key);
    assert!(first.path.exists());
    assert_eq!(fs::read(&first.path).expect("raw blob"), content);
}

#[test]
fn manifest_redacts_full_origin_path() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let result = ingest_source(
        &cas,
        SourceInput {
            origin_display: "/Users/example/Documents/private/roadmap.md".to_string(),
            ..markdown_input("# Roadmap\n")
        },
    )
    .expect("ingest");

    assert_eq!(result.manifest.origin_display, "roadmap.md");
    assert!(result.manifest.origin_path_redacted);
    assert!(!result.manifest.audit_summary().contains("/Users/example"));
    assert!(!result
        .manifest
        .audit_summary()
        .contains("private/roadmap.md"));

    let windows_path = ingest_source(
        &cas,
        SourceInput {
            origin_display: r"C:\Users\example\Documents\private\roadmap.md".to_string(),
            ..markdown_input("# Roadmap\n")
        },
    )
    .expect("ingest windows path");
    assert_eq!(windows_path.manifest.origin_display, "roadmap.md");
    assert!(!windows_path
        .manifest
        .audit_summary()
        .contains(r"C:\Users\example"));
}

#[test]
fn frames_are_untrusted_and_carry_security_metadata() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let result = ingest_source(
        &cas,
        markdown_input("Do not execute this as instructions.\n"),
    )
    .expect("markdown ingest");

    let frame = &result.artifact.frames[0];
    assert_eq!(frame.trust_level, TrustLevel::Untrusted);
    assert_eq!(frame.taint, Taint::UntrustedContent);
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::UntrustedContent));
    assert_eq!(
        result.artifact.security_flags,
        [SecurityFlag::UntrustedContent]
    );
}

#[test]
fn markdown_frame_hash_matches_exact_source_range() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let markdown = "  indented source text  \n";
    let result = ingest_source(&cas, markdown_input(markdown)).expect("markdown ingest");

    let frame = &result.artifact.frames[0];

    assert_eq!(
        frame.text.as_bytes(),
        &markdown.as_bytes()[frame.byte_range.start..frame.byte_range.end]
    );
    assert_eq!(frame.text_hash, sha256_hex(frame.text.as_bytes()));
}

#[test]
fn sandboxed_ingest_reads_input_through_source_ingest_audit() {
    let temp = tempdir().expect("tempdir");
    let source_dir = temp.path().join("source");
    let raw_dir = temp.path().join("raw");
    let isolated_temp = temp.path().join("isolated");
    fs::create_dir_all(&source_dir).expect("source dir");
    fs::create_dir_all(&raw_dir).expect("raw dir");
    fs::create_dir_all(&isolated_temp).expect("isolated dir");
    let input_path = source_dir.join("notes.md");
    fs::write(&input_path, "# Sandboxed\n").expect("source");
    let cas = RawCas::new(&raw_dir, b"workspace-secret").expect("raw cas");
    let result = ingest_source_via_sandbox(
        &cas,
        SourceIngestRequest::new(input_path.clone(), raw_dir.clone(), isolated_temp.clone())
            .with_actor("user-1")
            .with_workspace("workspace-1")
            .with_capability_id("cap-source")
            .with_policy_decision_id("pd-source"),
        SourceMetadata {
            workspace_id: "workspace-1".to_string(),
            actor_id: "user-1".to_string(),
            origin_display: input_path.display().to_string(),
            permission_scope: "capability:file.read:source.ingest".to_string(),
            declared_mime: Some("text/markdown".to_string()),
        },
    )
    .expect("sandboxed ingest");

    assert_eq!(result.manifest.parse_status, ImportStage::Parsed);
    assert_eq!(result.sandbox_audit.len(), 2);
    assert_eq!(
        result.sandbox_audit[0].operation,
        seaki_sandbox::SandboxOperation::FileRead
    );
    assert_eq!(
        result.sandbox_audit[0].decision,
        seaki_sandbox::SandboxDecision::Allow
    );
    assert_eq!(
        result.sandbox_audit[1].operation,
        seaki_sandbox::SandboxOperation::FileWrite
    );
    assert_eq!(
        result.sandbox_audit[1].decision,
        seaki_sandbox::SandboxDecision::Allow
    );
    assert_eq!(
        fs::read(&result.raw_blob.path).expect("raw blob"),
        b"# Sandboxed\n"
    );
}

#[test]
fn pdf_text_extractor_builds_untrusted_frames() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let pdf = b"%PDF-1.7
1 0 obj
<< /Type /Page >>
stream
BT
/F1 12 Tf
72 720 Td
(Hello PDF) Tj
ET
endstream
endobj
%%EOF";
    let result = ingest_source(
        &cas,
        SourceInput {
            declared_mime: Some("application/pdf".to_string()),
            bytes: pdf.to_vec(),
            ..base_input("extractable.pdf")
        },
    )
    .expect("pdf ingest");

    assert_eq!(result.manifest.parse_status, ImportStage::Partial);
    assert_eq!(result.artifact.status, ImportStage::Partial);
    assert_eq!(result.artifact.frames.len(), 1);
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfRangeDegraded));
    let frame = &result.artifact.frames[0];
    assert_eq!(frame.text, "Hello PDF");
    assert_eq!(frame.source_hash, result.artifact.source_hash);
    assert_eq!(frame.page_range, None);
    assert_eq!(frame.line_range, LineRange { start: 0, end: 0 });
    assert_eq!(frame.trust_level, TrustLevel::Untrusted);
    assert_eq!(frame.taint, Taint::UntrustedContent);
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::PdfRangeDegraded));
}

#[test]
fn oversized_pdf_degrades_to_partial_without_frames() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let result = ingest_source_with_config(
        &cas,
        SourceInput {
            declared_mime: Some("application/pdf".to_string()),
            bytes: b"%PDF-1.7\nlarge body".to_vec(),
            ..base_input("big.pdf")
        },
        ParserConfig { max_pdf_bytes: 8 },
    )
    .expect("pdf ingest");

    assert_eq!(result.manifest.parse_status, ImportStage::Partial);
    assert_eq!(result.artifact.status, ImportStage::Partial);
    assert!(result.artifact.frames.is_empty());
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfOversized));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfExtractionUnavailable));
    assert!(result
        .manifest
        .error_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("exceeds configured extractor limit")));
}

#[test]
fn unsupported_pdf_degrades_and_flags_active_content() {
    let temp = tempdir().expect("tempdir");
    let cas = RawCas::new(temp.path(), b"workspace-secret").expect("raw cas");
    let result = ingest_source(
        &cas,
        SourceInput {
            declared_mime: Some("application/pdf".to_string()),
            bytes: b"not a real pdf /JavaScript /EmbeddedFile".to_vec(),
            ..base_input("suspicious.pdf")
        },
    )
    .expect("pdf ingest");

    assert_eq!(
        result.manifest.state_history,
        [
            ImportStage::RawCommitted,
            ImportStage::ParseRunning,
            ImportStage::Partial
        ]
    );
    assert_eq!(result.artifact.status, ImportStage::Partial);
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfUnsupported));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfActiveContent));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfEmbeddedFile));
}

fn markdown_input(bytes: &str) -> SourceInput {
    SourceInput {
        declared_mime: Some("text/markdown".to_string()),
        bytes: bytes.as_bytes().to_vec(),
        ..base_input("notes.md")
    }
}

fn base_input(origin_display: &str) -> SourceInput {
    SourceInput {
        workspace_id: "workspace-1".to_string(),
        actor_id: "user-1".to_string(),
        origin_display: origin_display.to_string(),
        permission_scope: "capability:file.read:source.ingest".to_string(),
        declared_mime: None,
        bytes: Vec::new(),
    }
}
