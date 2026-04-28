use std::time::SystemTime;

use seaki_wiki::{
    ApprovalRequest, ApprovalStatus, ByteRange, Citation, Claim, ClaimConfidence, ClaimStatus,
    ConceptPage, LineRange, MimeSniff, ParsedFrame, SecurityFlag, SourceIngestState,
    SourceManifest, SourceVisibility, Taint, TrustLevel, TypedPage, WikiIndexStatus,
    WikiPatchError, WikiPatchProposal, WikiPatchStore, MARKDOWN_PARSER_VERSION,
    PARSED_FRAME_SCHEMA_HASH, SOURCE_MANIFEST_SCHEMA_HASH,
};

#[test]
fn rejects_citation_when_source_or_frame_does_not_exist_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal =
        proposal_with_citation(citation("citation-1", "missing-source", "frame-1", 0, 6));
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("missing source must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::InvalidCitation { citation_id, .. } if citation_id == "citation-1"
    ));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_citation_when_range_exceeds_frame_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 64));
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("out-of-frame citation range must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::InvalidCitation { citation_id, .. } if citation_id == "citation-1"
    ));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_new_citation_to_tombstoned_source_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Tombstoned)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("new citation to tombstoned source must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::TombstonedSource { source_id, .. } if source_id == "source-1"
    ));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_base_revision_conflict_without_mutating_store() {
    let mut store = WikiPatchStore::new(8);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let mut proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    proposal.base_revision = 7;
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("stale base revision must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::BaseRevisionConflict {
            expected_revision: 8,
            actual_revision: 7,
            ..
        }
    ));
    assert_store_unchanged(&store, 8);
}

#[test]
fn rejects_missing_approval_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));

    let error = store
        .commit_patch(proposal, None, &sources, &frames)
        .expect_err("missing approval must reject the patch");

    assert!(matches!(error, WikiPatchError::ApprovalRequired { .. }));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_pending_approval_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    let approval = approval_request(ApprovalStatus::Pending);

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("pending approval must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::ApprovalNotGranted {
            status: ApprovalStatus::Pending,
            ..
        }
    ));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_denied_approval_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    let approval = approval_request(ApprovalStatus::Denied);

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("denied approval must reject the patch");

    assert!(matches!(
        error,
        WikiPatchError::ApprovalNotGranted {
            status: ApprovalStatus::Denied,
            ..
        }
    ));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_citation_claim_mismatch_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let mut proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    proposal.citations[0].claim_id = "other-claim".to_string();
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("citation claim mismatch must reject the patch");

    assert!(matches!(error, WikiPatchError::InvalidProposal { .. }));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_active_claim_without_declared_citation_without_mutating_store() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let mut proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    proposal.claims[0].citation_ids.clear();
    proposal.citations.clear();
    let approval = approved_request();

    let error = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect_err("claim without citation must reject the patch");

    assert!(matches!(error, WikiPatchError::InvalidProposal { .. }));
    assert_store_unchanged(&store, 7);
}

#[test]
fn rejects_restricted_or_cross_workspace_source_without_mutating_store() {
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];

    let mut restricted_store = WikiPatchStore::new(7);
    let restricted_sources = vec![source_manifest("source-1", SourceVisibility::Restricted)];
    let error = restricted_store
        .commit_patch(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(approved_request()),
            &restricted_sources,
            &frames,
        )
        .expect_err("restricted source must reject the patch");
    assert!(matches!(error, WikiPatchError::InvalidCitation { .. }));
    assert_store_unchanged(&restricted_store, 7);

    let mut cross_workspace_store = WikiPatchStore::new(7);
    let mut cross_workspace_source = source_manifest("source-1", SourceVisibility::Visible);
    cross_workspace_source.workspace_id = "workspace-2".to_string();
    let error = cross_workspace_store
        .commit_patch(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(approved_request()),
            &[cross_workspace_source],
            &frames,
        )
        .expect_err("cross-workspace source must reject the patch");
    assert!(matches!(error, WikiPatchError::InvalidCitation { .. }));
    assert_store_unchanged(&cross_workspace_store, 7);
}

#[test]
fn rejects_approval_scope_or_decision_mismatch_without_mutating_store() {
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];

    let mut workspace_store = WikiPatchStore::new(7);
    let mut workspace_approval = approved_request();
    workspace_approval.workspace_id = "workspace-2".to_string();
    let error = workspace_store
        .commit_patch(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(workspace_approval),
            &sources,
            &frames,
        )
        .expect_err("cross-workspace approval must reject the patch");
    assert!(matches!(
        error,
        WikiPatchError::ApprovalScopeMismatch {
            field: "workspace_id",
            ..
        }
    ));
    assert_store_unchanged(&workspace_store, 7);

    let mut requester_store = WikiPatchStore::new(7);
    let mut requester_approval = approved_request();
    requester_approval.requested_by = "other-agent".to_string();
    let error = requester_store
        .commit_patch(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(requester_approval),
            &sources,
            &frames,
        )
        .expect_err("approval requested by another actor must reject the patch");
    assert!(matches!(
        error,
        WikiPatchError::ApprovalScopeMismatch {
            field: "requested_by",
            ..
        }
    ));
    assert_store_unchanged(&requester_store, 7);

    let mut decision_store = WikiPatchStore::new(7);
    let mut decision_approval = approved_request();
    decision_approval.decided_by = None;
    let error = decision_store
        .commit_patch(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(decision_approval),
            &sources,
            &frames,
        )
        .expect_err("approved decision without decider must reject the patch");
    assert!(matches!(
        error,
        WikiPatchError::ApprovalDecisionMissing { .. }
    ));
    assert_store_unchanged(&decision_store, 7);
}

#[test]
fn wal_append_failure_keeps_store_unchanged() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];

    let error = store
        .commit_patch_with_wal(
            proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6)),
            Some(approved_request()),
            &sources,
            &frames,
            |_| Err("wal unavailable"),
        )
        .expect_err("wal append failure must reject the patch");

    assert!(matches!(error, WikiPatchError::WalAppendFailed { .. }));
    assert_store_unchanged(&store, 7);
}

#[test]
fn commits_valid_patch_and_records_page_claim_citation_audit_and_stale_index() {
    let mut store = WikiPatchStore::new(7);
    let sources = vec![source_manifest("source-1", SourceVisibility::Visible)];
    let frames = vec![parsed_frame("source-1", "frame-1", 0, 18)];
    let proposal = proposal_with_citation(citation("citation-1", "source-1", "frame-1", 0, 6));
    let approval = approved_request();

    let transaction = store
        .commit_patch(proposal, Some(approval), &sources, &frames)
        .expect("valid patch should commit");

    assert_eq!(store.current_revision(), 8);
    assert_eq!(transaction.base_revision, 7);
    assert_eq!(transaction.committed_revision, 8);
    assert_eq!(transaction.patch_id, "patch-1");
    assert!(transaction.rollback_marker.is_some());
    assert!(store.rollback_marker("patch-1").is_some());
    assert!(store.transaction("patch-1").is_some());

    let page = store.page("concept:wiki-transaction").expect("page");
    assert!(
        matches!(page, TypedPage::Concept(ConceptPage { page_id, .. }) if page_id == "concept:wiki-transaction")
    );

    let claim = store.claim("claim-1").expect("claim");
    assert_eq!(claim.status, ClaimStatus::Active);
    assert_eq!(claim.confidence, ClaimConfidence::High);

    let registry_entry = store
        .citation_registry()
        .get("citation-1")
        .expect("citation registry entry");
    assert_eq!(registry_entry.claim_id, "claim-1");
    assert_eq!(registry_entry.source_id, "source-1");
    assert_eq!(registry_entry.frame_id.as_deref(), Some("frame-1"));

    assert!(store
        .audit_records()
        .iter()
        .any(|record| record.patch_id == "patch-1" && record.committed_revision == 8));
    assert_eq!(store.index_status(), WikiIndexStatus::Stale);
}

fn proposal_with_citation(citation: Citation) -> WikiPatchProposal {
    WikiPatchProposal {
        patch_id: "patch-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        actor_id: "agent-1".to_string(),
        base_revision: 7,
        page: TypedPage::Concept(ConceptPage {
            page_id: "concept:wiki-transaction".to_string(),
            title: "Wiki patch transaction".to_string(),
            definition: "A transaction that validates cited claims before committing wiki state."
                .to_string(),
            source_cards: vec!["source-1".to_string()],
            annotations: vec!["requires approval and rollback marker".to_string()],
            temporal_context: Some("M0".to_string()),
            supersedes: Vec::new(),
        }),
        claims: vec![Claim {
            claim_id: "claim-1".to_string(),
            page_id: "concept:wiki-transaction".to_string(),
            text: "Wiki patches commit only after citation validation.".to_string(),
            confidence: ClaimConfidence::High,
            status: ClaimStatus::Active,
            citation_ids: vec![citation.citation_id.clone()],
        }],
        citations: vec![citation],
        risk_summary: "headless test approval only; no user approval UI in M0-06".to_string(),
    }
}

fn citation(
    citation_id: &str,
    source_id: &str,
    frame_id: &str,
    start: usize,
    end: usize,
) -> Citation {
    Citation {
        citation_id: citation_id.to_string(),
        claim_id: "claim-1".to_string(),
        source_id: source_id.to_string(),
        frame_id: Some(frame_id.to_string()),
        byte_range: ByteRange { start, end },
        line_range: Some(LineRange { start: 1, end: 1 }),
        quote: "Wiki".to_string(),
    }
}

fn approved_request() -> ApprovalRequest {
    approval_request(ApprovalStatus::Approved)
}

fn approval_request(status: ApprovalStatus) -> ApprovalRequest {
    ApprovalRequest {
        approval_id: "approval-1".to_string(),
        patch_id: "patch-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        requested_by: "agent-1".to_string(),
        decided_by: Some("reviewer-1".to_string()),
        status,
        decided_at: Some(SystemTime::UNIX_EPOCH),
    }
}

fn source_manifest(source_id: &str, visibility: SourceVisibility) -> SourceManifest {
    SourceManifest {
        source_id: source_id.to_string(),
        workspace_id: "workspace-1".to_string(),
        actor_id: "user-1".to_string(),
        origin_display: "notes.md".to_string(),
        origin_path_redacted: true,
        mime: "text/markdown".to_string(),
        size: 18,
        raw_key: "workspace-keyed-raw-key".to_string(),
        raw_content_hash: "raw-content-hash".to_string(),
        permission_scope: "capability:file.read:source.ingest".to_string(),
        parse_status: SourceIngestState::Parsed,
        state_history: vec![
            SourceIngestState::RawCommitted,
            SourceIngestState::ParseRunning,
            SourceIngestState::Parsed,
        ],
        schema_hash: SOURCE_MANIFEST_SCHEMA_HASH.to_string(),
        imported_at: SystemTime::UNIX_EPOCH,
        tombstoned_at: (visibility == SourceVisibility::Tombstoned)
            .then_some(SystemTime::UNIX_EPOCH),
        visibility,
        error_summary: None,
    }
}

fn parsed_frame(source_id: &str, frame_id: &str, start: usize, end: usize) -> ParsedFrame {
    ParsedFrame {
        frame_id: frame_id.to_string(),
        source_id: source_id.to_string(),
        source_hash: "raw-content-hash".to_string(),
        parser_version: MARKDOWN_PARSER_VERSION.to_string(),
        page_range: None,
        line_range: LineRange { start: 1, end: 1 },
        byte_range: ByteRange { start, end },
        mime_sniff: MimeSniff {
            declared: Some("text/markdown".to_string()),
            sniffed: "text/markdown".to_string(),
        },
        text: "Wiki transaction".to_string(),
        text_hash: "frame-text-hash".to_string(),
        trust_level: TrustLevel::Untrusted,
        taint: Taint::UntrustedContent,
        schema_hash: PARSED_FRAME_SCHEMA_HASH.to_string(),
        security_flags: vec![SecurityFlag::UntrustedContent],
    }
}

fn assert_store_unchanged(store: &WikiPatchStore, revision: u64) {
    assert_eq!(store.current_revision(), revision);
    assert!(store.audit_records().is_empty());
    assert!(store.citation_registry().is_empty());
    assert_eq!(store.index_status(), WikiIndexStatus::Fresh);
}
