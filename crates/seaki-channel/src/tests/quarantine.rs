use super::*;
use std::time::SystemTime;
use tempfile::TempDir;

fn sample_attachment() -> ChannelAttachmentRef {
    ChannelAttachmentRef {
        attachment_id: "att-unit".to_string(),
        provider: "slack".to_string(),
        provider_tenant_id: "tenant-1".to_string(),
        provider_chat_id: "chat-1".to_string(),
        provider_message_id: "msg-1".to_string(),
        provider_thread_id: "thread-1".to_string(),
        provider_file_key: "key-1".to_string(),
        provider_file_version: "v1".to_string(),
        original_name: "doc.txt".to_string(),
        declared_mime: "text/plain".to_string(),
        declared_size: 5,
        content_hash: None,
        download_capability_required: false,
    }
}

#[test]
fn pipeline_clean_for_txt_file() {
    let tmp = TempDir::new().unwrap();
    let content = b"hello";
    let downloader = FakeAttachmentDownloader::new_with_content(content.to_vec());
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let mut att = sample_attachment();
    att.original_name = "doc.txt".to_string();
    att.declared_mime = "text/plain".to_string();

    let result = pipeline.process(&att);
    assert!(
        matches!(result, QuarantineResult::Clean(ref q) if q.observed_mime == "text/plain"),
        "expected Clean for txt, got {result:?}"
    );
}

#[test]
fn pipeline_audit_timestamp_is_set() {
    let tmp = TempDir::new().unwrap();
    let downloader = FakeAttachmentDownloader::new_with_content(vec![1, 2, 3]);
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let _ = pipeline.process(&sample_attachment());
    let audits = pipeline.audit_log();
    assert!(!audits.is_empty());
    assert!(audits[0].timestamp > SystemTime::UNIX_EPOCH);
}

#[test]
fn pipeline_audit_for_unknown_attachment_is_empty() {
    let tmp = TempDir::new().unwrap();
    let downloader = FakeAttachmentDownloader::new_with_content(vec![0; 10]);
    let pipeline = QuarantinePipeline::new(downloader, tmp.path());

    let _ = pipeline.process(&sample_attachment());
    let audits = pipeline.audit_for_attachment("nonexistent");
    assert!(audits.is_empty());
}
