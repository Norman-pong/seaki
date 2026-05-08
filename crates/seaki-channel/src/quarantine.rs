//! Quarantine pipeline: download, validate metadata, malware scan stub, audit.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::grant::{ChannelAttachmentRef, MalwareScanStatus, QuarantinedDownload};

/// Result of running an attachment through the quarantine pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineResult {
    Clean(QuarantinedDownload),
    SizeMismatch { declared: u64, observed: u64 },
    MimeMismatch { declared: String, observed: String },
    HashMismatch { declared: String, observed: String },
    Suspicious(String),
    Infected(String),
    IOError(String),
}

/// A single quarantine audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineAuditRecord {
    pub attachment_id: String,
    pub provider_file_key: String,
    pub stage: QuarantineStage,
    pub result: QuarantineResultSummary,
    pub timestamp: SystemTime,
}

/// Stage of quarantine processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineStage {
    Download,
    HashCheck,
    MimeCheck,
    MalwareScan,
}

/// Summary outcome for a single stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineResultSummary {
    Passed,
    Failed(String),
}

/// Trait for attachment download implementations.
pub trait AttachmentDownloader: Send + Sync {
    /// Download the attachment identified by `attachment` into `dest`.
    fn download(&self, attachment: &ChannelAttachmentRef, dest: &Path) -> Result<(), String>;
}

/// In-memory fake downloader that writes predefined content to the destination.
pub struct FakeAttachmentDownloader {
    content: Vec<u8>,
}

impl FakeAttachmentDownloader {
    /// Create a fake downloader that writes `content` on each download call.
    #[must_use]
    pub fn new_with_content(content: Vec<u8>) -> Self {
        Self { content }
    }
}

impl AttachmentDownloader for FakeAttachmentDownloader {
    fn download(&self, _attachment: &ChannelAttachmentRef, dest: &Path) -> Result<(), String> {
        let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
        file.write_all(&self.content).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Quarantine pipeline that downloads, validates and optionally scans attachments.
#[derive(Debug)]
pub struct QuarantinePipeline<D: AttachmentDownloader> {
    downloader: D,
    quarantine_root: PathBuf,
    audit_log: Mutex<Vec<QuarantineAuditRecord>>,
}

impl<D: AttachmentDownloader> QuarantinePipeline<D> {
    /// Create a new pipeline.
    #[must_use]
    pub fn new(downloader: D, quarantine_root: impl Into<PathBuf>) -> Self {
        Self {
            downloader,
            quarantine_root: quarantine_root.into(),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    /// Process an attachment through the full quarantine pipeline.
    ///
    /// Steps:
    /// 1. Download to `{quarantine_root}/{provider_file_key}_{provider_file_version}`.
    /// 2. Compute `observed_size`.
    /// 3. Compute SHA-256 `content_hash`.
    /// 4. Validate `declared_size` vs `observed_size`.
    /// 5. Validate `declared_mime` vs observed MIME (simple extension stub).
    /// 6. If `content_hash` is provided, validate hash consistency.
    /// 7. Malware scan stub (passes if hash/mime are consistent).
    /// 8. Return `Clean(...)` or the appropriate failure variant.
    pub fn process(&self, attachment: &ChannelAttachmentRef) -> QuarantineResult {
        let dest = {
            let base = format!(
                "{}_{}",
                attachment.provider_file_key, attachment.provider_file_version
            );
            if let Some(ext) = Path::new(&attachment.original_name)
                .extension()
                .and_then(|e| e.to_str())
            {
                self.quarantine_root.join(format!("{base}.{ext}"))
            } else {
                self.quarantine_root.join(base)
            }
        };

        // Step 1: Download
        if let Err(e) = self.downloader.download(attachment, &dest) {
            self.record_audit(
                attachment,
                QuarantineStage::Download,
                QuarantineResultSummary::Failed(e.clone()),
            );
            return QuarantineResult::IOError(e);
        }
        self.record_audit(
            attachment,
            QuarantineStage::Download,
            QuarantineResultSummary::Passed,
        );

        // Step 2: Observed size
        let observed_size = match fs::metadata(&dest) {
            Ok(m) => m.len(),
            Err(e) => {
                let msg = e.to_string();
                self.record_audit(
                    attachment,
                    QuarantineStage::Download,
                    QuarantineResultSummary::Failed(msg.clone()),
                );
                return QuarantineResult::IOError(msg);
            }
        };

        // Step 3: SHA-256 hash
        let observed_hash = match compute_sha256(&dest) {
            Ok(h) => h,
            Err(e) => {
                let msg = e.to_string();
                self.record_audit(
                    attachment,
                    QuarantineStage::HashCheck,
                    QuarantineResultSummary::Failed(msg.clone()),
                );
                return QuarantineResult::IOError(msg);
            }
        };

        // Step 4: Size check
        if attachment.declared_size != 0 && observed_size != attachment.declared_size {
            self.record_audit(
                attachment,
                QuarantineStage::HashCheck,
                QuarantineResultSummary::Failed(format!(
                    "size mismatch: declared {}, observed {}",
                    attachment.declared_size, observed_size
                )),
            );
            return QuarantineResult::SizeMismatch {
                declared: attachment.declared_size,
                observed: observed_size,
            };
        }
        self.record_audit(
            attachment,
            QuarantineStage::HashCheck,
            QuarantineResultSummary::Passed,
        );

        // Step 5: MIME check (stub via extension)
        let observed_mime = observed_mime_from_path(&dest)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        if !attachment.declared_mime.is_empty()
            && attachment.declared_mime.to_lowercase() != observed_mime.to_lowercase()
        {
            self.record_audit(
                attachment,
                QuarantineStage::MimeCheck,
                QuarantineResultSummary::Failed(format!(
                    "mime mismatch: declared {}, observed {}",
                    attachment.declared_mime, observed_mime
                )),
            );
            return QuarantineResult::MimeMismatch {
                declared: attachment.declared_mime.clone(),
                observed: observed_mime,
            };
        }
        self.record_audit(
            attachment,
            QuarantineStage::MimeCheck,
            QuarantineResultSummary::Passed,
        );

        // Step 6: Hash consistency (if provider supplied a hash)
        if let Some(ref declared_hash) = attachment.content_hash {
            if declared_hash != &observed_hash {
                self.record_audit(
                    attachment,
                    QuarantineStage::HashCheck,
                    QuarantineResultSummary::Failed(format!(
                        "hash mismatch: declared {declared_hash}, observed {observed_hash}"
                    )),
                );
                return QuarantineResult::HashMismatch {
                    declared: declared_hash.clone(),
                    observed: observed_hash,
                };
            }
        }

        // Step 7: Malware scan stub
        // For the stub, hash/mime consistency means Clean.
        self.record_audit(
            attachment,
            QuarantineStage::MalwareScan,
            QuarantineResultSummary::Passed,
        );

        QuarantineResult::Clean(QuarantinedDownload {
            file_key: attachment.provider_file_key.clone(),
            version: attachment.provider_file_version.clone(),
            quarantine_path: dest.to_string_lossy().to_string(),
            observed_mime,
            content_hash: observed_hash,
            malware_scan_status: MalwareScanStatus::Clean,
            observed_size,
        })
    }

    /// Return a clone of the full audit log.
    pub fn audit_log(&self) -> Vec<QuarantineAuditRecord> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Return audit records for a specific attachment ID.
    pub fn audit_for_attachment(&self, attachment_id: &str) -> Vec<QuarantineAuditRecord> {
        self.audit_log
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.attachment_id == attachment_id)
            .cloned()
            .collect()
    }

    fn record_audit(
        &self,
        attachment: &ChannelAttachmentRef,
        stage: QuarantineStage,
        result: QuarantineResultSummary,
    ) {
        let record = QuarantineAuditRecord {
            attachment_id: attachment.attachment_id.clone(),
            provider_file_key: attachment.provider_file_key.clone(),
            stage,
            result,
            timestamp: SystemTime::now(),
        };
        let mut log = self.audit_log.lock().unwrap();
        log.push(record);
    }
}

fn compute_sha256(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let result = hasher.finalize();
    let hex = result
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{hex}"))
}

fn observed_mime_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "json" => "application/json",
            "html" | "htm" => "text/html",
            "js" => "application/javascript",
            "css" => "text/css",
            "zip" => "application/zip",
            "mp4" => "video/mp4",
            "mp3" => "audio/mpeg",
            _ => "application/octet-stream",
        })
        .map(|s| s.to_string())
}
