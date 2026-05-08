use seaki_channel::grant::{ChannelAttachmentRef, MalwareScanStatus};
use seaki_channel::quarantine::{
    AttachmentDownloader, FakeAttachmentDownloader, QuarantinePipeline, QuarantineResult,
    QuarantineStage,
};
use tempfile::TempDir;

fn sample_attachment() -> ChannelAttachmentRef {
    ChannelAttachmentRef {
        attachment_id: "att-1".to_string(),
        provider: "slack".to_string(),
        provider_tenant_id: "tenant-1".to_string(),
        provider_chat_id: "chat-1".to_string(),
        provider_message_id: "msg-1".to_string(),
        provider_thread_id: "thread-1".to_string(),
        provider_file_key: "key-1".to_string(),
        provider_file_version: "v1".to_string(),
        original_name: "photo.png".to_string(),
        declared_mime: "image/png".to_string(),
        declared_size: 13,
        content_hash: None,
        download_capability_required: false,
    }
}

fn sample_attachment_with_hash(hash: &str) -> ChannelAttachmentRef {
    ChannelAttachmentRef {
        content_hash: Some(hash.to_string()),
        ..sample_attachment()
    }
}

fn sample_attachment_with_mime(mime: &str) -> ChannelAttachmentRef {
    ChannelAttachmentRef {
        declared_mime: mime.to_string(),
        ..sample_attachment()
    }
}

fn sample_attachment_with_size(size: u64) -> ChannelAttachmentRef {
    ChannelAttachmentRef {
        declared_size: size,
        ..sample_attachment()
    }
}

fn compute_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(data);
    let hex = result
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

// ---- Success cases ----

#[test]
fn pipeline_clean_result() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let result = pipeline.process(&sample_attachment());
    assert!(
        matches!(result, QuarantineResult::Clean(ref q) if q.observed_size == 13),
        "expected Clean, got {result:?}"
    );

    if let QuarantineResult::Clean(q) = result {
        assert_eq!(q.file_key, "key-1");
        assert_eq!(q.version, "v1");
        assert_eq!(q.observed_mime, "image/png");
        assert_eq!(q.malware_scan_status, MalwareScanStatus::Clean);
        assert!(q.quarantine_path.contains("key-1_v1"));
    }
}

#[test]
fn pipeline_audit_log_has_all_stages() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let _ = pipeline.process(&sample_attachment());
    let audits = pipeline.audit_for_attachment("att-1");
    assert_eq!(audits.len(), 4);
    assert!(matches!(audits[0].stage, QuarantineStage::Download));
    assert!(matches!(audits[1].stage, QuarantineStage::HashCheck));
    assert!(matches!(audits[2].stage, QuarantineStage::MimeCheck));
    assert!(matches!(audits[3].stage, QuarantineStage::MalwareScan));
}

#[test]
fn pipeline_observed_size_matches_content() {
    let tmp = TempDir::new().unwrap();
    let content = vec![0u8; 4096];
    let downloader = FakeAttachmentDownloader::new_with_content(content.clone());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let mut att = sample_attachment();
    att.declared_size = 4096;

    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::Clean(ref q) if q.observed_size == 4096),
        "expected Clean with size 4096, got {result:?}"
    );
}

#[test]
fn pipeline_hash_matches_when_provided() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let expected_hash = compute_sha256(content);
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let att = sample_attachment_with_hash(&expected_hash);
    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::Clean(ref q) if q.content_hash == expected_hash),
        "expected Clean with matching hash, got {result:?}"
    );
}

// ---- Failure cases ----

#[test]
fn pipeline_size_mismatch_detected() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let att = sample_attachment_with_size(999);
    let result = pipeline.process(&att);
    assert!(
        matches!(
            result,
            QuarantineResult::SizeMismatch {
                declared: 999,
                observed: 13
            }
        ),
        "expected SizeMismatch, got {result:?}"
    );
}

#[test]
fn pipeline_mime_mismatch_detected() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    // The fake file path ends in .png, so observed mime is image/png.
    let att = sample_attachment_with_mime("application/pdf");
    let result = pipeline.process(&att);
    assert!(
        matches!(
            result,
            QuarantineResult::MimeMismatch {
                ref declared,
                ref observed,
            } if declared == "application/pdf" && observed == "image/png"
        ),
        "expected MimeMismatch, got {result:?}"
    );
}

#[test]
fn pipeline_hash_mismatch_detected() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let att = sample_attachment_with_hash(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::HashMismatch { .. }),
        "expected HashMismatch, got {result:?}"
    );
}

#[test]
fn pipeline_declared_size_zero_skips_size_check() {
    let tmp = TempDir::new().unwrap();
    let content = b"any content";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let mut att = sample_attachment();
    att.declared_size = 0;

    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::Clean(..)),
        "expected Clean when declared_size is 0, got {result:?}"
    );
}

#[test]
fn pipeline_declared_mime_empty_skips_mime_check() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let mut att = sample_attachment();
    att.declared_mime = String::new();

    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::Clean(..)),
        "expected Clean when declared_mime is empty, got {result:?}"
    );
}

#[test]
fn pipeline_audit_log_records_failures() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello, world!";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let att = sample_attachment_with_size(999);
    let _ = pipeline.process(&att);
    let audits = pipeline.audit_for_attachment("att-1");
    // Download passed, HashCheck (size) failed.
    assert_eq!(audits.len(), 2);
    assert!(matches!(audits[0].stage, QuarantineStage::Download));
    assert!(matches!(audits[1].stage, QuarantineStage::HashCheck));
}

#[test]
fn fake_downloader_writes_content() {
    let tmp = TempDir::new().unwrap();
    let content = b"custom payload";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());

    let att = sample_attachment();
    let dest = tmp.path().join("test_file");
    downloader.download(&att, &dest).unwrap();

    let read_back = std::fs::read(&dest).unwrap();
    assert_eq!(read_back, content);
}
