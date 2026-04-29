use std::fs;
use std::path::{Path, PathBuf};

use seaki_sandbox::{SandboxDecision, SandboxOperation, SandboxProfile, SourceIngestRequest};
use seaki_wiki::{
    ingest_source_via_sandbox, ingest_source_via_sandbox_with_config, ByteRange, LineRange,
    ParserConfig, RawCas, SecurityFlag, SourceIngestState, SourceMetadata, SourceVisibility, Taint,
    TrustLevel, WikiError, MARKDOWN_PARSER_VERSION, PARSED_FRAME_SCHEMA_HASH, PDF_PARSER_VERSION,
    SOURCE_MANIFEST_SCHEMA_HASH,
};
use sha2::{Digest, Sha256};
use tempfile::{tempdir, TempDir};

#[test]
fn sandboxed_markdown_ingest_exposes_manifest_and_parsed_frame_metadata() {
    let fixture = IngestFixture::new();
    let markdown = "# Title\n\nHello [link](https://example.com).\n";
    let source_path = fixture.write_source("notes.md", markdown.as_bytes());
    let result = fixture.ingest_path(
        &source_path,
        Some("text/markdown"),
        Some(ParserConfig::default()),
    );

    assert_eq!(result.manifest.workspace_id, "workspace-1");
    assert_eq!(result.manifest.actor_id, "user-1");
    assert_eq!(result.manifest.origin_display, "notes.md");
    assert!(result.manifest.origin_path_redacted);
    assert_eq!(result.manifest.mime, "text/markdown");
    assert_eq!(result.manifest.size, markdown.len() as u64);
    assert_eq!(result.manifest.raw_key, result.raw_blob.raw_key);
    assert_eq!(
        result.manifest.raw_content_hash,
        sha256_hex(markdown.as_bytes())
    );
    assert_ne!(result.manifest.raw_key, result.manifest.raw_content_hash);
    assert_eq!(
        result.manifest.permission_scope,
        "capability:file.read:source.ingest"
    );
    assert_eq!(result.manifest.parse_status, SourceIngestState::Parsed);
    assert_eq!(
        result.manifest.state_history,
        [
            SourceIngestState::RawCommitted,
            SourceIngestState::ParseRunning,
            SourceIngestState::Parsed
        ]
    );
    assert_eq!(result.manifest.schema_hash, SOURCE_MANIFEST_SCHEMA_HASH);
    assert_eq!(result.manifest.visibility, SourceVisibility::Visible);
    assert!(result.manifest.error_summary.is_none());

    assert_eq!(result.artifact.source_id, result.manifest.source_id);
    assert_eq!(
        result.artifact.source_hash,
        result.manifest.raw_content_hash
    );
    assert_eq!(result.artifact.parser_version, MARKDOWN_PARSER_VERSION);
    assert_eq!(result.artifact.status, SourceIngestState::Parsed);
    assert_eq!(
        result.artifact.security_flags,
        [SecurityFlag::UntrustedContent]
    );

    let frame = &result.artifact.frames[1];
    assert_eq!(frame.source_id, result.manifest.source_id);
    assert_eq!(frame.source_hash, result.artifact.source_hash);
    assert_eq!(frame.parser_version, MARKDOWN_PARSER_VERSION);
    assert_eq!(frame.page_range, None);
    assert_eq!(frame.line_range, LineRange { start: 3, end: 3 });
    assert_eq!(frame.byte_range, ByteRange { start: 9, end: 43 });
    assert_eq!(
        frame.text.as_bytes(),
        &markdown.as_bytes()[frame.byte_range.start..frame.byte_range.end]
    );
    assert_eq!(frame.text_hash, sha256_hex(frame.text.as_bytes()));
    assert_eq!(frame.trust_level, TrustLevel::Untrusted);
    assert_eq!(frame.taint, Taint::UntrustedContent);
    assert_eq!(frame.schema_hash, PARSED_FRAME_SCHEMA_HASH);
    assert_eq!(frame.mime_sniff.declared.as_deref(), Some("text/markdown"));
    assert_eq!(frame.mime_sniff.sniffed, "text/markdown");
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::UntrustedContent));
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::ExternalReference));
}

#[test]
fn sandboxed_markdown_ingest_records_input_read_audit_and_raw_cas_commit() {
    let fixture = IngestFixture::new();
    let source_path = fixture.write_source("audit.md", b"# Audit\n");
    let result = fixture.ingest_path(&source_path, Some("text/markdown"), None);

    assert_eq!(result.sandbox_audit.len(), 2);
    let read_audit = &result.sandbox_audit[0];
    assert_eq!(read_audit.profile, SandboxProfile::SourceIngest);
    assert_eq!(read_audit.operation, SandboxOperation::FileRead);
    assert_eq!(read_audit.decision, SandboxDecision::Allow);
    assert_eq!(read_audit.actor_id, "user-1");
    assert_eq!(read_audit.workspace_id, "workspace-1");
    assert_eq!(read_audit.capability_id.as_deref(), Some("cap-source"));
    assert_eq!(read_audit.policy_decision_id.as_deref(), Some("pd-source"));
    let canonical_source = fs::canonicalize(&source_path).expect("canonical source");
    assert_eq!(read_audit.path.as_deref(), Some(canonical_source.as_path()));

    let canonical_raw_dir = fs::canonicalize(&fixture.raw_dir).expect("canonical raw dir");
    let write_audit = &result.sandbox_audit[1];
    assert_eq!(write_audit.profile, SandboxProfile::SourceIngest);
    assert_eq!(write_audit.operation, SandboxOperation::FileWrite);
    assert_eq!(write_audit.decision, SandboxDecision::Allow);
    assert!(write_audit
        .path
        .as_deref()
        .is_some_and(|path| path.starts_with(&canonical_raw_dir)));

    assert!(result.raw_blob.newly_written);
    assert!(result.raw_blob.path.starts_with(&canonical_raw_dir));
    assert_eq!(
        fs::read(&result.raw_blob.path).expect("raw blob"),
        b"# Audit\n"
    );
}

#[test]
fn sandboxed_pdf_ingest_keeps_extractable_text_untrusted() {
    let fixture = IngestFixture::new();
    let source_path = fixture.write_source("extractable.pdf", extractable_pdf());
    let result = fixture.ingest_path(&source_path, Some("application/pdf"), None);

    assert_eq!(result.manifest.parse_status, SourceIngestState::Partial);
    assert_eq!(result.artifact.status, SourceIngestState::Partial);
    assert_eq!(result.artifact.parser_version, PDF_PARSER_VERSION);
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfRangeDegraded));
    assert_eq!(result.artifact.frames.len(), 1);

    let frame = &result.artifact.frames[0];
    assert_eq!(frame.text, "Hello PDF");
    assert_eq!(frame.text_hash, sha256_hex(b"Hello PDF"));
    assert_eq!(frame.page_range, None);
    assert_eq!(frame.line_range, LineRange { start: 0, end: 0 });
    assert_eq!(frame.trust_level, TrustLevel::Untrusted);
    assert_eq!(frame.taint, Taint::UntrustedContent);
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::UntrustedContent));
    assert!(frame
        .security_flags
        .contains(&SecurityFlag::PdfRangeDegraded));
}

#[test]
fn sandboxed_pdf_ingest_degrades_oversized_active_content_without_frames() {
    let fixture = IngestFixture::new();
    let source_path = fixture.write_source(
        "suspicious.pdf",
        b"%PDF-1.7\n/JavaScript /OpenAction /EmbeddedFile\n",
    );
    let result = fixture.ingest_path(
        &source_path,
        Some("application/pdf"),
        Some(ParserConfig { max_pdf_bytes: 8 }),
    );

    assert_eq!(result.manifest.parse_status, SourceIngestState::Partial);
    assert_eq!(result.artifact.status, SourceIngestState::Partial);
    assert!(result.artifact.frames.is_empty());
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfOversized));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfActiveContent));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfEmbeddedFile));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfExtractionUnavailable));
    assert!(result
        .artifact
        .security_flags
        .contains(&SecurityFlag::PdfNeedsOcr));
    assert!(result
        .manifest
        .error_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("active content was not executed")));
}

#[test]
fn sandboxed_ingest_rejects_metadata_workspace_mismatch_before_raw_write() {
    let fixture = IngestFixture::new();
    let source_path = fixture.write_source("mismatch.md", b"# Mismatch\n");
    let request = fixture.request(&source_path);
    let mut metadata = fixture.metadata(&source_path, Some("text/markdown"));
    metadata.workspace_id = "workspace-2".to_string();
    let cas = fixture.raw_cas();

    let error = ingest_source_via_sandbox(&cas, request, metadata).expect_err("mismatch");

    assert!(matches!(
        error,
        WikiError::SourceMetadataMismatch {
            field: "workspace_id",
            ..
        }
    ));
    assert_raw_dir_empty(&fixture.raw_dir);
}

#[test]
fn sandboxed_ingest_rejects_raw_cas_root_mismatch_before_raw_write() {
    let fixture = IngestFixture::new();
    let other_raw_dir = fixture._temp.path().join("other-raw");
    fs::create_dir_all(&other_raw_dir).expect("other raw dir");
    let source_path = fixture.write_source("root.md", b"# Root\n");
    let cas = RawCas::try_new(&other_raw_dir, b"workspace-secret").expect("raw cas");

    let error = ingest_source_via_sandbox(
        &cas,
        fixture.request(&source_path),
        fixture.metadata(&source_path, Some("text/markdown")),
    )
    .expect_err("root mismatch");

    assert!(matches!(error, WikiError::RawCasRootMismatch { .. }));
    assert_raw_dir_empty(&fixture.raw_dir);
}

#[test]
fn sandboxed_ingest_rejects_missing_input_before_raw_write() {
    let fixture = IngestFixture::new();
    let missing_path = fixture.source_dir.join("missing.md");

    let error = ingest_source_via_sandbox(
        &fixture.raw_cas(),
        fixture.request(&missing_path),
        fixture.metadata(&missing_path, Some("text/markdown")),
    )
    .expect_err("missing input");

    assert!(matches!(error, WikiError::Sandbox(_)));
    assert_raw_dir_empty(&fixture.raw_dir);
}

#[test]
fn sandboxed_binary_ingest_commits_raw_and_marks_parse_failed() {
    let fixture = IngestFixture::new();
    let source_path = fixture.write_source("binary.bin", &[0xff, 0x00, 0xfe]);
    let result = fixture.ingest_path(&source_path, Some("application/octet-stream"), None);

    assert_eq!(result.manifest.parse_status, SourceIngestState::Failed);
    assert_eq!(result.artifact.status, SourceIngestState::Failed);
    assert!(result.artifact.frames.is_empty());
    assert!(result
        .manifest
        .error_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("unsupported source mime")));
    assert_eq!(result.sandbox_audit.len(), 2);
    assert_eq!(
        result.sandbox_audit[1].operation,
        SandboxOperation::FileWrite
    );
}

struct IngestFixture {
    _temp: TempDir,
    source_dir: PathBuf,
    raw_dir: PathBuf,
    isolated_temp_dir: PathBuf,
}

impl IngestFixture {
    fn new() -> Self {
        let temp = tempdir().expect("tempdir");
        let source_dir = temp.path().join("source");
        let raw_dir = temp.path().join("raw");
        let isolated_temp_dir = temp.path().join("isolated");
        fs::create_dir_all(&source_dir).expect("source dir");
        fs::create_dir_all(&raw_dir).expect("raw dir");
        fs::create_dir_all(&isolated_temp_dir).expect("isolated temp dir");

        Self {
            _temp: temp,
            source_dir,
            raw_dir,
            isolated_temp_dir,
        }
    }

    fn write_source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.source_dir.join(name);
        fs::write(&path, bytes).expect("source write");
        path
    }

    fn ingest_path(
        &self,
        source_path: &Path,
        declared_mime: Option<&str>,
        config: Option<ParserConfig>,
    ) -> seaki_wiki::SourceIngestResult {
        let request = self.request(source_path);
        let metadata = self.metadata(source_path, declared_mime);
        let cas = self.raw_cas();

        match config {
            Some(config) => ingest_source_via_sandbox_with_config(&cas, request, metadata, config)
                .expect("ingest"),
            None => ingest_source_via_sandbox(&cas, request, metadata).expect("ingest"),
        }
    }

    fn request(&self, source_path: &Path) -> SourceIngestRequest {
        SourceIngestRequest::new(source_path, &self.raw_dir, &self.isolated_temp_dir)
            .with_actor("user-1")
            .with_workspace("workspace-1")
            .with_capability_id("cap-source")
            .with_policy_decision_id("pd-source")
    }

    fn metadata(&self, source_path: &Path, declared_mime: Option<&str>) -> SourceMetadata {
        SourceMetadata {
            workspace_id: "workspace-1".to_string(),
            actor_id: "user-1".to_string(),
            origin_display: source_path.display().to_string(),
            permission_scope: "capability:file.read:source.ingest".to_string(),
            declared_mime: declared_mime.map(str::to_string),
        }
    }

    fn raw_cas(&self) -> RawCas {
        RawCas::try_new(&self.raw_dir, b"workspace-secret").expect("raw cas")
    }
}

fn extractable_pdf() -> &'static [u8] {
    b"%PDF-1.7
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
%%EOF"
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn assert_raw_dir_empty(raw_dir: &Path) {
    assert_eq!(
        fs::read_dir(raw_dir).expect("raw dir").count(),
        0,
        "raw CAS dir should remain empty"
    );
}
