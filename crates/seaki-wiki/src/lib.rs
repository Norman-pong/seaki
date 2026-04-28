use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use seaki_dto::ImportStage;
use seaki_sandbox::{
    run_source_ingest, SandboxAuditRecord, SandboxError, SourceIngestContext, SourceIngestRequest,
};
use sha2::{Digest, Sha256};

mod patch;

pub use patch::{
    ApprovalRequest, ApprovalStatus, AuditRecord, Citation, CitationRegistryEntry, Claim,
    ClaimConfidence, ClaimStatus, ConceptPage, RollbackMarker, TypedPage, WikiIndexStatus,
    WikiPatchError, WikiPatchProposal, WikiPatchStore, WikiPatchTransaction, WikiPatchWalRecord,
};

pub const RAW_SOURCE_STORAGE: &str = "append-only-content-addressed";
pub const SOURCE_MANIFEST_SCHEMA_HASH: &str = "source-manifest.v1";
pub const PARSED_FRAME_SCHEMA_HASH: &str = "parsed-frame.v1";
pub const MARKDOWN_PARSER_VERSION: &str = "seaki-markdown-parser.v1";
pub const PDF_PARSER_VERSION: &str = "seaki-pdf-parser.degraded.v1";
pub const DEFAULT_MAX_PDF_BYTES: usize = 16 * 1024 * 1024;

pub use seaki_dto::ImportStage as SourceIngestState;

#[derive(Debug)]
pub enum WikiError {
    Io(io::Error),
    Sandbox(SandboxError),
    RawCasCollision {
        raw_key: String,
    },
    EmptyWorkspaceKey,
    RawCasRootMismatch {
        expected_raw_cas_dir: PathBuf,
        actual_raw_cas_dir: PathBuf,
    },
    SourceMetadataMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for WikiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Sandbox(error) => write!(f, "{error}"),
            Self::RawCasCollision { raw_key } => {
                write!(f, "raw CAS key collision for key {raw_key}")
            }
            Self::EmptyWorkspaceKey => write!(f, "workspace key must not be empty"),
            Self::RawCasRootMismatch {
                expected_raw_cas_dir,
                actual_raw_cas_dir,
            } => write!(
                f,
                "raw CAS root {} does not match sandbox raw CAS dir {}",
                actual_raw_cas_dir.display(),
                expected_raw_cas_dir.display()
            ),
            Self::SourceMetadataMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "source metadata {field} mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for WikiError {}

impl From<io::Error> for WikiError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<SandboxError> for WikiError {
    fn from(value: SandboxError) -> Self {
        Self::Sandbox(value)
    }
}

pub type WikiResult<T> = Result<T, WikiError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBlob {
    pub raw_key: String,
    pub content_hash: String,
    pub len: u64,
    pub path: PathBuf,
    pub newly_written: bool,
}

#[derive(Debug, Clone)]
pub struct RawCas {
    root: PathBuf,
    workspace_key: Vec<u8>,
}

impl RawCas {
    pub fn new(root: impl AsRef<Path>, workspace_key: impl AsRef<[u8]>) -> WikiResult<Self> {
        let workspace_key = workspace_key.as_ref().to_vec();
        if workspace_key.is_empty() {
            return Err(WikiError::EmptyWorkspaceKey);
        }

        Ok(Self {
            root: root.as_ref().to_path_buf(),
            workspace_key,
        })
    }

    pub fn append(&self, content: &[u8]) -> WikiResult<RawBlob> {
        let content_hash = sha256_hex(content);
        let raw_key = workspace_keyed_digest(&self.workspace_key, &content_hash);
        let path = self.path_for_key(&raw_key);

        if path.exists() {
            let existing = fs::read(&path)?;
            if sha256_hex(&existing) != content_hash {
                return Err(WikiError::RawCasCollision { raw_key });
            }

            return Ok(RawBlob {
                raw_key,
                content_hash,
                len: content.len() as u64,
                path,
                newly_written: false,
            });
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => file.write_all(content)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&path)?;
                if sha256_hex(&existing) != content_hash {
                    return Err(WikiError::RawCasCollision { raw_key });
                }

                return Ok(RawBlob {
                    raw_key,
                    content_hash,
                    len: content.len() as u64,
                    path,
                    newly_written: false,
                });
            }
            Err(error) => return Err(error.into()),
        }

        Ok(RawBlob {
            raw_key,
            content_hash,
            len: content.len() as u64,
            path,
            newly_written: true,
        })
    }

    fn append_with_sandbox(
        &self,
        context: &mut SourceIngestContext,
        content: &[u8],
    ) -> WikiResult<RawBlob> {
        let content_hash = sha256_hex(content);
        let raw_key = workspace_keyed_digest(&self.workspace_key, &content_hash);
        let path = self.path_for_key(&raw_key);

        if path.exists() {
            let existing = fs::read(&path)?;
            if sha256_hex(&existing) != content_hash {
                return Err(WikiError::RawCasCollision { raw_key });
            }

            return Ok(RawBlob {
                raw_key,
                content_hash,
                len: content.len() as u64,
                path,
                newly_written: false,
            });
        }

        let written_path = match context.write_raw_cas(raw_relative_path(&raw_key), content) {
            Ok(path) => path,
            Err(SandboxError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&path)?;
                if sha256_hex(&existing) != content_hash {
                    return Err(WikiError::RawCasCollision { raw_key });
                }

                return Ok(RawBlob {
                    raw_key,
                    content_hash,
                    len: content.len() as u64,
                    path,
                    newly_written: false,
                });
            }
            Err(error) => return Err(error.into()),
        };

        Ok(RawBlob {
            raw_key,
            content_hash,
            len: content.len() as u64,
            path: written_path,
            newly_written: true,
        })
    }

    pub fn path_for_key(&self, raw_key: &str) -> PathBuf {
        self.root.join(raw_relative_path(raw_key))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInput {
    pub workspace_id: String,
    pub actor_id: String,
    pub origin_display: String,
    pub permission_scope: String,
    pub declared_mime: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub workspace_id: String,
    pub actor_id: String,
    pub origin_display: String,
    pub permission_scope: String,
    pub declared_mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifest {
    pub source_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub origin_display: String,
    pub origin_path_redacted: bool,
    pub mime: String,
    pub size: u64,
    pub raw_key: String,
    pub raw_content_hash: String,
    pub permission_scope: String,
    pub parse_status: ImportStage,
    pub state_history: Vec<ImportStage>,
    pub schema_hash: String,
    pub imported_at: SystemTime,
    pub tombstoned_at: Option<SystemTime>,
    pub visibility: SourceVisibility,
    pub error_summary: Option<String>,
}

impl SourceManifest {
    pub fn audit_summary(&self) -> String {
        format!(
            "source_id={} origin_display={} mime={} size={} parse_status={}",
            self.source_id,
            self.origin_display,
            self.mime,
            self.size,
            self.parse_status.as_str()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceVisibility {
    Visible,
    Restricted,
    Tombstoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArtifact {
    pub source_id: String,
    pub source_hash: String,
    pub parser_version: String,
    pub status: ImportStage,
    pub frames: Vec<ParsedFrame>,
    pub security_flags: Vec<SecurityFlag>,
    pub generated_at: SystemTime,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFrame {
    pub frame_id: String,
    pub source_id: String,
    pub source_hash: String,
    pub parser_version: String,
    pub page_range: Option<PageRange>,
    pub line_range: LineRange,
    pub byte_range: ByteRange,
    pub mime_sniff: MimeSniff,
    pub text: String,
    pub text_hash: String,
    pub trust_level: TrustLevel,
    pub taint: Taint,
    pub schema_hash: String,
    pub security_flags: Vec<SecurityFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeSniff {
    pub declared: Option<String>,
    pub sniffed: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taint {
    UntrustedContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityFlag {
    UntrustedContent,
    ExternalReference,
    PdfExtractionUnavailable,
    PdfOversized,
    PdfUnsupported,
    PdfActiveContent,
    PdfEmbeddedFile,
    PdfNeedsOcr,
    PdfRangeDegraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIngestResult {
    pub manifest: SourceManifest,
    pub artifact: ParsedArtifact,
    pub raw_blob: RawBlob,
    pub sandbox_audit: Vec<SandboxAuditRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserConfig {
    pub max_pdf_bytes: usize,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            max_pdf_bytes: DEFAULT_MAX_PDF_BYTES,
        }
    }
}

pub fn ingest_source_via_sandbox(
    cas: &RawCas,
    request: SourceIngestRequest,
    metadata: SourceMetadata,
) -> WikiResult<SourceIngestResult> {
    ingest_source_via_sandbox_with_config(cas, request, metadata, ParserConfig::default())
}

pub fn ingest_source_via_sandbox_with_config(
    cas: &RawCas,
    request: SourceIngestRequest,
    metadata: SourceMetadata,
    config: ParserConfig,
) -> WikiResult<SourceIngestResult> {
    ensure_sandbox_request_matches_metadata(&request, &metadata)?;
    ensure_raw_cas_root_matches_request(cas, &request)?;
    let permission_scope = source_ingest_permission_scope(&request);

    let run = run_source_ingest(request, |context| {
        let bytes = context.read_input()?;
        let raw_blob = cas.append_with_sandbox(context, &bytes);
        Ok((bytes, raw_blob))
    })?;
    let audit = run.audit;
    let (bytes, raw_blob) = run.output?;
    let raw_blob = raw_blob?;

    let source = SourceInput {
        workspace_id: metadata.workspace_id,
        actor_id: metadata.actor_id,
        origin_display: metadata.origin_display,
        permission_scope,
        declared_mime: metadata.declared_mime,
        bytes,
    };

    let mut result = ingest_committed_source(raw_blob, source, config);
    result.sandbox_audit = audit;
    Ok(result)
}

#[cfg(test)]
fn ingest_source(cas: &RawCas, input: SourceInput) -> WikiResult<SourceIngestResult> {
    ingest_source_with_config(cas, input, ParserConfig::default())
}

#[cfg(test)]
fn ingest_source_with_config(
    cas: &RawCas,
    input: SourceInput,
    config: ParserConfig,
) -> WikiResult<SourceIngestResult> {
    let raw_blob = cas.append(&input.bytes)?;
    Ok(ingest_committed_source(raw_blob, input, config))
}

fn ingest_committed_source(
    raw_blob: RawBlob,
    input: SourceInput,
    config: ParserConfig,
) -> SourceIngestResult {
    let sniffed_mime = sniff_mime(input.declared_mime.as_deref(), &input.bytes);
    let source_id = source_id_for(&raw_blob.raw_key);
    let (origin_display, origin_path_redacted) = redact_origin_display(&input.origin_display);
    let mut manifest = SourceManifest {
        source_id: source_id.clone(),
        workspace_id: input.workspace_id,
        actor_id: input.actor_id,
        origin_display,
        origin_path_redacted,
        mime: sniffed_mime.clone(),
        size: raw_blob.len,
        raw_key: raw_blob.raw_key.clone(),
        raw_content_hash: raw_blob.content_hash.clone(),
        permission_scope: input.permission_scope,
        parse_status: ImportStage::RawCommitted,
        state_history: vec![ImportStage::RawCommitted],
        schema_hash: SOURCE_MANIFEST_SCHEMA_HASH.to_string(),
        imported_at: SystemTime::now(),
        tombstoned_at: None,
        visibility: SourceVisibility::Visible,
        error_summary: None,
    };

    manifest.parse_status = ImportStage::ParseRunning;
    manifest.state_history.push(ImportStage::ParseRunning);

    let artifact = parse_raw_source(
        &source_id,
        &raw_blob.content_hash,
        input.declared_mime,
        &sniffed_mime,
        &input.bytes,
        config,
    );

    manifest.parse_status = artifact.status;
    manifest.state_history.push(artifact.status);
    manifest.error_summary = artifact.error_summary.clone();

    SourceIngestResult {
        manifest,
        artifact,
        raw_blob,
        sandbox_audit: Vec::new(),
    }
}

fn parse_raw_source(
    source_id: &str,
    source_hash: &str,
    declared_mime: Option<String>,
    sniffed_mime: &str,
    bytes: &[u8],
    config: ParserConfig,
) -> ParsedArtifact {
    if is_markdown_mime(sniffed_mime) {
        return parse_markdown(source_id, source_hash, declared_mime, sniffed_mime, bytes);
    }

    if is_pdf_mime(sniffed_mime) || declared_mime.as_deref() == Some("application/pdf") {
        return parse_pdf_stub(
            source_id,
            source_hash,
            declared_mime,
            sniffed_mime,
            bytes,
            config,
        );
    }

    ParsedArtifact {
        source_id: source_id.to_string(),
        source_hash: source_hash.to_string(),
        parser_version: MARKDOWN_PARSER_VERSION.to_string(),
        status: ImportStage::Failed,
        frames: Vec::new(),
        security_flags: vec![SecurityFlag::UntrustedContent],
        generated_at: SystemTime::now(),
        error_summary: Some(format!("unsupported source mime: {sniffed_mime}")),
    }
}

fn parse_markdown(
    source_id: &str,
    source_hash: &str,
    declared_mime: Option<String>,
    sniffed_mime: &str,
    bytes: &[u8],
) -> ParsedArtifact {
    let Ok(markdown) = std::str::from_utf8(bytes) else {
        return ParsedArtifact {
            source_id: source_id.to_string(),
            source_hash: source_hash.to_string(),
            parser_version: MARKDOWN_PARSER_VERSION.to_string(),
            status: ImportStage::Failed,
            frames: Vec::new(),
            security_flags: vec![SecurityFlag::UntrustedContent],
            generated_at: SystemTime::now(),
            error_summary: Some("markdown source is not valid utf-8".to_string()),
        };
    };

    let frames = markdown_paragraphs(markdown)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let text = markdown[range.byte_range.start..range.byte_range.end].to_string();
            let mut security_flags = vec![SecurityFlag::UntrustedContent];
            if contains_external_reference(&text) {
                security_flags.push(SecurityFlag::ExternalReference);
            }

            ParsedFrame {
                frame_id: frame_id_for(source_id, index, &text),
                source_id: source_id.to_string(),
                source_hash: source_hash.to_string(),
                parser_version: MARKDOWN_PARSER_VERSION.to_string(),
                page_range: None,
                line_range: range.line_range,
                byte_range: range.byte_range,
                mime_sniff: MimeSniff {
                    declared: declared_mime.clone(),
                    sniffed: sniffed_mime.to_string(),
                },
                text_hash: sha256_hex(text.as_bytes()),
                text,
                trust_level: TrustLevel::Untrusted,
                taint: Taint::UntrustedContent,
                schema_hash: PARSED_FRAME_SCHEMA_HASH.to_string(),
                security_flags,
            }
        })
        .collect::<Vec<_>>();

    ParsedArtifact {
        source_id: source_id.to_string(),
        source_hash: source_hash.to_string(),
        parser_version: MARKDOWN_PARSER_VERSION.to_string(),
        status: ImportStage::Parsed,
        frames,
        security_flags: vec![SecurityFlag::UntrustedContent],
        generated_at: SystemTime::now(),
        error_summary: None,
    }
}

fn parse_pdf_stub(
    source_id: &str,
    source_hash: &str,
    declared_mime: Option<String>,
    sniffed_mime: &str,
    bytes: &[u8],
    config: ParserConfig,
) -> ParsedArtifact {
    let mut flags = vec![SecurityFlag::UntrustedContent];
    let mut reasons = Vec::new();

    if bytes.len() > config.max_pdf_bytes {
        flags.push(SecurityFlag::PdfOversized);
        reasons.push(format!(
            "pdf exceeds configured extractor limit: {} > {} bytes",
            bytes.len(),
            config.max_pdf_bytes
        ));
    }

    if !bytes.starts_with(b"%PDF-") {
        flags.push(SecurityFlag::PdfUnsupported);
        reasons.push("pdf magic header missing or unsupported".to_string());
    }

    if pdf_contains_any_case_insensitive(
        bytes,
        &[b"/javascript", b"/js", b"/launch", b"/openaction"],
    ) {
        flags.push(SecurityFlag::PdfActiveContent);
        reasons.push("pdf active content was not executed".to_string());
    }

    if pdf_contains_any_case_insensitive(bytes, &[b"/embeddedfile", b"/filespec"]) {
        flags.push(SecurityFlag::PdfEmbeddedFile);
        reasons.push("pdf embedded files were ignored".to_string());
    }

    let spans = if bytes.starts_with(b"%PDF-") && bytes.len() <= config.max_pdf_bytes {
        pdf_text_spans(bytes)
    } else {
        Vec::new()
    };

    if !spans.is_empty() {
        flags.push(SecurityFlag::PdfRangeDegraded);
        reasons
            .push("pdf text ranges do not include reliable page or line coordinates".to_string());
    }

    let frame_security_flags = flags.clone();
    let frames = spans
        .into_iter()
        .enumerate()
        .map(|(index, span)| ParsedFrame {
            frame_id: frame_id_for(source_id, index, &span.text),
            source_id: source_id.to_string(),
            source_hash: source_hash.to_string(),
            parser_version: PDF_PARSER_VERSION.to_string(),
            page_range: None,
            line_range: LineRange { start: 0, end: 0 },
            byte_range: span.byte_range,
            mime_sniff: MimeSniff {
                declared: declared_mime.clone(),
                sniffed: sniffed_mime.to_string(),
            },
            text_hash: sha256_hex(span.text.as_bytes()),
            text: span.text,
            trust_level: TrustLevel::Untrusted,
            taint: Taint::UntrustedContent,
            schema_hash: PARSED_FRAME_SCHEMA_HASH.to_string(),
            security_flags: frame_security_flags.clone(),
        })
        .collect::<Vec<_>>();

    if frames.is_empty() {
        flags.push(SecurityFlag::PdfExtractionUnavailable);
        flags.push(SecurityFlag::PdfNeedsOcr);
        reasons.push("pdf text extraction produced no text frames".to_string());
    }
    let status = if frames.is_empty() || flags.len() > 1 {
        ImportStage::Partial
    } else {
        ImportStage::Parsed
    };

    ParsedArtifact {
        source_id: source_id.to_string(),
        source_hash: source_hash.to_string(),
        parser_version: PDF_PARSER_VERSION.to_string(),
        status,
        frames,
        security_flags: flags,
        generated_at: SystemTime::now(),
        error_summary: if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PdfTextSpan {
    text: String,
    byte_range: ByteRange,
}

fn pdf_text_spans(bytes: &[u8]) -> Vec<PdfTextSpan> {
    let mut spans = Vec::new();
    let mut in_text_object = false;
    let mut index = 0_usize;

    while index < bytes.len() {
        if token_at(bytes, index, b"BT") {
            in_text_object = true;
            index += 2;
            continue;
        }
        if token_at(bytes, index, b"ET") {
            in_text_object = false;
            index += 2;
            continue;
        }

        if in_text_object && bytes[index] == b'(' {
            if let Some((text, byte_range, next_index)) = parse_pdf_literal_string(bytes, index) {
                if !text.trim().is_empty() {
                    spans.push(PdfTextSpan { text, byte_range });
                }
                index = next_index;
                continue;
            }
        }

        index += 1;
    }

    spans
}

fn parse_pdf_literal_string(bytes: &[u8], start: usize) -> Option<(String, ByteRange, usize)> {
    let mut output = Vec::new();
    let mut depth = 1_i32;
    let mut index = start + 1;
    let content_start = index;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            let next = *bytes.get(index + 1)?;
            match next {
                b'n' => output.push(b'\n'),
                b'r' => output.push(b'\r'),
                b't' => output.push(b'\t'),
                b'b' => output.push(0x08),
                b'f' => output.push(0x0c),
                b'(' | b')' | b'\\' => output.push(next),
                b'\n' | b'\r' => {}
                other => output.push(other),
            }
            index += 2;
            continue;
        }

        if byte == b'(' {
            depth += 1;
            output.push(byte);
            index += 1;
            continue;
        }

        if byte == b')' {
            depth -= 1;
            if depth == 0 {
                return Some((
                    String::from_utf8_lossy(&output).into_owned(),
                    ByteRange {
                        start: content_start,
                        end: index,
                    },
                    index + 1,
                ));
            }
            output.push(byte);
            index += 1;
            continue;
        }

        output.push(byte);
        index += 1;
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarkdownRange {
    line_range: LineRange,
    byte_range: ByteRange,
}

fn markdown_paragraphs(markdown: &str) -> Vec<MarkdownRange> {
    let mut ranges = Vec::new();
    let mut paragraph_start_byte = None;
    let mut paragraph_start_line = 1_u32;
    let mut paragraph_end_byte = 0_usize;
    let mut byte_offset = 0_usize;
    let mut current_line = 1_u32;

    for line_with_newline in markdown.split_inclusive('\n') {
        let line = line_with_newline
            .trim_end_matches('\n')
            .trim_end_matches('\r');
        let line_start = byte_offset;
        let line_end = line_start + line.len();

        if line.trim().is_empty() {
            if let Some(start_byte) = paragraph_start_byte.take() {
                ranges.push(MarkdownRange {
                    line_range: LineRange {
                        start: paragraph_start_line,
                        end: current_line.saturating_sub(1),
                    },
                    byte_range: ByteRange {
                        start: start_byte,
                        end: paragraph_end_byte,
                    },
                });
            }
        } else {
            if paragraph_start_byte.is_none() {
                paragraph_start_byte = Some(line_start);
                paragraph_start_line = current_line;
            }
            paragraph_end_byte = line_end;
        }

        byte_offset += line_with_newline.len();
        current_line += 1;

        if line_end == markdown.len() && !line.trim().is_empty() {
            break;
        }
    }

    if let Some(start_byte) = paragraph_start_byte {
        ranges.push(MarkdownRange {
            line_range: LineRange {
                start: paragraph_start_line,
                end: current_line.saturating_sub(1),
            },
            byte_range: ByteRange {
                start: start_byte,
                end: paragraph_end_byte,
            },
        });
    }

    ranges
}

fn sniff_mime(declared_mime: Option<&str>, bytes: &[u8]) -> String {
    if bytes.starts_with(b"%PDF-") {
        return "application/pdf".to_string();
    }

    if let Some(mime) = declared_mime {
        if is_markdown_mime(mime) || mime == "text/plain" || mime == "application/pdf" {
            return mime.to_string();
        }
    }

    if std::str::from_utf8(bytes).is_ok() {
        return "text/markdown".to_string();
    }

    "application/octet-stream".to_string()
}

fn is_markdown_mime(mime: &str) -> bool {
    matches!(mime, "text/markdown" | "text/x-markdown" | "text/plain")
}

fn is_pdf_mime(mime: &str) -> bool {
    mime == "application/pdf"
}

fn contains_external_reference(text: &str) -> bool {
    text.contains("http://") || text.contains("https://")
}

fn pdf_contains_any_case_insensitive(bytes: &[u8], lower_needles: &[&[u8]]) -> bool {
    let lower = bytes
        .iter()
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();

    lower_needles
        .iter()
        .any(|needle| contains_bytes(&lower, needle))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn token_at(bytes: &[u8], index: usize, token: &[u8]) -> bool {
    bytes[index..].starts_with(token)
        && index
            .checked_sub(1)
            .and_then(|previous| bytes.get(previous))
            .is_none_or(|byte| byte.is_ascii_whitespace())
        && bytes
            .get(index + token.len())
            .is_none_or(|byte| byte.is_ascii_whitespace())
}

fn ensure_sandbox_request_matches_metadata(
    request: &SourceIngestRequest,
    metadata: &SourceMetadata,
) -> WikiResult<()> {
    ensure_metadata_field_matches(
        "workspace_id",
        &request.workspace_id,
        &metadata.workspace_id,
    )?;
    ensure_metadata_field_matches("actor_id", &request.actor_id, &metadata.actor_id)
}

fn ensure_metadata_field_matches(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> WikiResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(WikiError::SourceMetadataMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn ensure_raw_cas_root_matches_request(
    cas: &RawCas,
    request: &SourceIngestRequest,
) -> WikiResult<()> {
    let actual_raw_cas_dir = fs::canonicalize(&cas.root)?;
    let expected_raw_cas_dir = fs::canonicalize(&request.raw_cas_dir)?;
    if actual_raw_cas_dir == expected_raw_cas_dir {
        Ok(())
    } else {
        Err(WikiError::RawCasRootMismatch {
            expected_raw_cas_dir,
            actual_raw_cas_dir,
        })
    }
}

fn source_ingest_permission_scope(_request: &SourceIngestRequest) -> String {
    "capability:file.read:source.ingest".to_string()
}

fn raw_relative_path(raw_key: &str) -> PathBuf {
    PathBuf::from(&raw_key[..2]).join(raw_key)
}

fn source_id_for(raw_key: &str) -> String {
    format!("src_{}", &raw_key[..32])
}

fn frame_id_for(source_id: &str, index: usize, text: &str) -> String {
    let text_hash = sha256_hex(text.as_bytes());
    format!("{source_id}:frame:{index}:{}", &text_hash[..16])
}

fn workspace_keyed_digest(workspace_key: &[u8], content_hash: &str) -> String {
    let mut message = Vec::with_capacity(b"seaki.raw-cas.v1".len() + 1 + content_hash.len());
    message.extend_from_slice(b"seaki.raw-cas.v1");
    message.push(0);
    message.extend_from_slice(content_hash.as_bytes());

    hmac_sha256_hex(workspace_key, &message)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0_u8; BLOCK_SIZE];

    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; BLOCK_SIZE];
    let mut outer_pad = [0x5c_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);

    hex_lower(&outer.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn redact_origin_display(origin_display: &str) -> (String, bool) {
    let path = Path::new(origin_display);
    let has_path_separator = path.components().count() > 1
        || origin_display.contains('/')
        || origin_display.contains('\\');

    let path_file_name = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .next_back()
        .filter(|name| !name.is_empty());
    let separator_file_name = origin_display
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty());
    let file_name = separator_file_name.or(path_file_name);

    match (file_name, has_path_separator) {
        (Some(name), true) => (name.to_string(), true),
        _ => (origin_display.to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
