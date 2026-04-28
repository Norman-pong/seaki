use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::SystemTime;

use crate::{ByteRange, LineRange, ParsedFrame, SourceManifest, SourceVisibility};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPatchProposal {
    pub patch_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub base_revision: u64,
    pub page: TypedPage,
    pub claims: Vec<Claim>,
    pub citations: Vec<Citation>,
    pub risk_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedPage {
    Concept(ConceptPage),
}

impl TypedPage {
    pub fn page_id(&self) -> &str {
        match self {
            Self::Concept(page) => &page.page_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptPage {
    pub page_id: String,
    pub title: String,
    pub definition: String,
    pub source_cards: Vec<String>,
    pub annotations: Vec<String>,
    pub temporal_context: Option<String>,
    pub supersedes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub claim_id: String,
    pub page_id: String,
    pub text: String,
    pub confidence: ClaimConfidence,
    pub status: ClaimStatus,
    pub citation_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimStatus {
    Proposed,
    Active,
    Superseded,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub citation_id: String,
    pub claim_id: String,
    pub source_id: String,
    pub frame_id: Option<String>,
    pub byte_range: ByteRange,
    pub line_range: Option<LineRange>,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub patch_id: String,
    pub workspace_id: String,
    pub requested_by: String,
    pub decided_by: Option<String>,
    pub status: ApprovalStatus,
    pub decided_at: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPatchTransaction {
    pub transaction_id: String,
    pub patch_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub base_revision: u64,
    pub committed_revision: u64,
    pub page_id: String,
    pub claim_ids: Vec<String>,
    pub citation_ids: Vec<String>,
    pub approval_id: String,
    pub rollback_marker: Option<RollbackMarker>,
    pub audit_record_id: String,
    pub risk_summary: String,
    pub committed_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackMarker {
    pub rollback_marker_id: String,
    pub patch_id: String,
    pub transaction_id: String,
    pub previous_revision: u64,
    pub affected_page_ids: Vec<String>,
    pub affected_claim_ids: Vec<String>,
    pub affected_citation_ids: Vec<String>,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRegistryEntry {
    pub citation_id: String,
    pub claim_id: String,
    pub source_id: String,
    pub frame_id: Option<String>,
    pub byte_range: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub audit_record_id: String,
    pub transaction_id: String,
    pub patch_id: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub base_revision: u64,
    pub committed_revision: u64,
    pub rollback_marker_id: String,
    pub recorded_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPatchWalRecord {
    pub transaction_id: String,
    pub patch_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub committed_revision: u64,
    pub rollback_marker_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiIndexStatus {
    Fresh,
    Stale,
}

#[derive(Debug, Clone)]
pub struct WikiPatchStore {
    current_revision: u64,
    pages: BTreeMap<String, TypedPage>,
    claims: BTreeMap<String, Claim>,
    citations: BTreeMap<String, Citation>,
    citation_registry: BTreeMap<String, CitationRegistryEntry>,
    transactions: BTreeMap<String, WikiPatchTransaction>,
    rollback_markers: BTreeMap<String, RollbackMarker>,
    audit_records: Vec<AuditRecord>,
    index_status: WikiIndexStatus,
}

impl WikiPatchStore {
    pub fn new(current_revision: u64) -> Self {
        Self {
            current_revision,
            pages: BTreeMap::new(),
            claims: BTreeMap::new(),
            citations: BTreeMap::new(),
            citation_registry: BTreeMap::new(),
            transactions: BTreeMap::new(),
            rollback_markers: BTreeMap::new(),
            audit_records: Vec::new(),
            index_status: WikiIndexStatus::Fresh,
        }
    }

    pub fn current_revision(&self) -> u64 {
        self.current_revision
    }

    pub fn page(&self, page_id: &str) -> Option<&TypedPage> {
        self.pages.get(page_id)
    }

    pub fn claim(&self, claim_id: &str) -> Option<&Claim> {
        self.claims.get(claim_id)
    }

    pub fn citation(&self, citation_id: &str) -> Option<&Citation> {
        self.citations.get(citation_id)
    }

    pub fn citation_registry(&self) -> &BTreeMap<String, CitationRegistryEntry> {
        &self.citation_registry
    }

    pub fn transaction(&self, patch_id: &str) -> Option<&WikiPatchTransaction> {
        self.transactions.get(patch_id)
    }

    pub fn rollback_marker(&self, patch_id: &str) -> Option<&RollbackMarker> {
        self.rollback_markers.get(patch_id)
    }

    pub fn audit_records(&self) -> &[AuditRecord] {
        &self.audit_records
    }

    pub fn index_status(&self) -> WikiIndexStatus {
        self.index_status
    }

    pub fn commit_patch(
        &mut self,
        proposal: WikiPatchProposal,
        approval: Option<ApprovalRequest>,
        sources: &[SourceManifest],
        frames: &[ParsedFrame],
    ) -> Result<WikiPatchTransaction, WikiPatchError> {
        self.commit_patch_with_wal(proposal, approval, sources, frames, |_| Ok::<_, String>(()))
    }

    pub fn commit_patch_with_wal<E: fmt::Display>(
        &mut self,
        proposal: WikiPatchProposal,
        approval: Option<ApprovalRequest>,
        sources: &[SourceManifest],
        frames: &[ParsedFrame],
        append_wal: impl FnOnce(&WikiPatchWalRecord) -> Result<(), E>,
    ) -> Result<WikiPatchTransaction, WikiPatchError> {
        self.validate_base_revision(&proposal)?;
        validate_proposal_shape(&proposal)?;
        let approval = validate_approval(&proposal, approval.as_ref())?;
        validate_citations(&proposal, sources, frames)?;

        let committed_revision = self.current_revision + 1;
        let committed_at = SystemTime::now();
        let transaction_id = format!("wiki-txn-{committed_revision}-{}", proposal.patch_id);
        let rollback_marker_id = format!("rollback-{transaction_id}");
        let audit_record_id = format!("audit-{transaction_id}");
        let page_id = proposal.page.page_id().to_string();
        let claim_ids = proposal
            .claims
            .iter()
            .map(|claim| claim.claim_id.clone())
            .collect::<Vec<_>>();
        let citation_ids = proposal
            .citations
            .iter()
            .map(|citation| citation.citation_id.clone())
            .collect::<Vec<_>>();
        let rollback_marker = RollbackMarker {
            rollback_marker_id: rollback_marker_id.clone(),
            patch_id: proposal.patch_id.clone(),
            transaction_id: transaction_id.clone(),
            previous_revision: proposal.base_revision,
            affected_page_ids: vec![page_id.clone()],
            affected_claim_ids: claim_ids.clone(),
            affected_citation_ids: citation_ids.clone(),
            created_at: committed_at,
        };
        let transaction = WikiPatchTransaction {
            transaction_id: transaction_id.clone(),
            patch_id: proposal.patch_id.clone(),
            workspace_id: proposal.workspace_id.clone(),
            actor_id: proposal.actor_id.clone(),
            base_revision: proposal.base_revision,
            committed_revision,
            page_id: page_id.clone(),
            claim_ids,
            citation_ids,
            approval_id: approval.approval_id.clone(),
            rollback_marker: Some(rollback_marker.clone()),
            audit_record_id: audit_record_id.clone(),
            risk_summary: proposal.risk_summary.clone(),
            committed_at,
        };
        let audit_record = AuditRecord {
            audit_record_id,
            transaction_id,
            patch_id: proposal.patch_id.clone(),
            actor_id: proposal.actor_id.clone(),
            workspace_id: proposal.workspace_id.clone(),
            base_revision: proposal.base_revision,
            committed_revision,
            rollback_marker_id: rollback_marker_id.clone(),
            recorded_at: committed_at,
        };
        let wal_record = WikiPatchWalRecord {
            transaction_id: transaction.transaction_id.clone(),
            patch_id: transaction.patch_id.clone(),
            workspace_id: transaction.workspace_id.clone(),
            actor_id: transaction.actor_id.clone(),
            committed_revision: transaction.committed_revision,
            rollback_marker_id: rollback_marker_id.clone(),
        };

        append_wal(&wal_record).map_err(|error| WikiPatchError::WalAppendFailed {
            patch_id: proposal.patch_id.clone(),
            reason: error.to_string(),
        })?;

        self.pages.insert(page_id, proposal.page);
        for claim in proposal.claims {
            self.claims.insert(claim.claim_id.clone(), claim);
        }
        for citation in proposal.citations {
            self.citation_registry.insert(
                citation.citation_id.clone(),
                CitationRegistryEntry {
                    citation_id: citation.citation_id.clone(),
                    claim_id: citation.claim_id.clone(),
                    source_id: citation.source_id.clone(),
                    frame_id: citation.frame_id.clone(),
                    byte_range: citation.byte_range,
                },
            );
            self.citations
                .insert(citation.citation_id.clone(), citation);
        }

        self.current_revision = committed_revision;
        self.transactions
            .insert(proposal.patch_id.clone(), transaction.clone());
        self.rollback_markers
            .insert(proposal.patch_id, rollback_marker);
        self.audit_records.push(audit_record);
        self.index_status = WikiIndexStatus::Stale;

        Ok(transaction)
    }

    fn validate_base_revision(&self, proposal: &WikiPatchProposal) -> Result<(), WikiPatchError> {
        if proposal.base_revision == self.current_revision {
            Ok(())
        } else {
            Err(WikiPatchError::BaseRevisionConflict {
                expected_revision: self.current_revision,
                actual_revision: proposal.base_revision,
                patch_id: proposal.patch_id.clone(),
            })
        }
    }
}

impl Default for WikiPatchStore {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikiPatchError {
    InvalidCitation {
        citation_id: String,
        reason: String,
    },
    TombstonedSource {
        citation_id: String,
        source_id: String,
    },
    BaseRevisionConflict {
        expected_revision: u64,
        actual_revision: u64,
        patch_id: String,
    },
    ApprovalRequired {
        patch_id: String,
    },
    ApprovalNotGranted {
        approval_id: String,
        patch_id: String,
        status: ApprovalStatus,
    },
    ApprovalPatchMismatch {
        approval_id: String,
        expected_patch_id: String,
        actual_patch_id: String,
    },
    ApprovalScopeMismatch {
        approval_id: String,
        field: &'static str,
        expected: String,
        actual: String,
    },
    ApprovalDecisionMissing {
        approval_id: String,
        patch_id: String,
    },
    WalAppendFailed {
        patch_id: String,
        reason: String,
    },
    InvalidProposal {
        reason: String,
    },
}

impl fmt::Display for WikiPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCitation {
                citation_id,
                reason,
            } => write!(f, "invalid citation {citation_id}: {reason}"),
            Self::TombstonedSource {
                citation_id,
                source_id,
            } => write!(
                f,
                "citation {citation_id} references tombstoned source {source_id}"
            ),
            Self::BaseRevisionConflict {
                expected_revision,
                actual_revision,
                patch_id,
            } => write!(
                f,
                "patch {patch_id} base revision conflict: expected {expected_revision}, got {actual_revision}"
            ),
            Self::ApprovalRequired { patch_id } => {
                write!(f, "approval is required for patch {patch_id}")
            }
            Self::ApprovalNotGranted {
                approval_id,
                patch_id,
                status,
            } => write!(
                f,
                "approval {approval_id} for patch {patch_id} is not granted: {status:?}"
            ),
            Self::ApprovalPatchMismatch {
                approval_id,
                expected_patch_id,
                actual_patch_id,
            } => write!(
                f,
                "approval {approval_id} targets patch {actual_patch_id}, expected {expected_patch_id}"
            ),
            Self::ApprovalScopeMismatch {
                approval_id,
                field,
                expected,
                actual,
            } => write!(
                f,
                "approval {approval_id} {field} mismatch: expected {expected}, got {actual}"
            ),
            Self::ApprovalDecisionMissing {
                approval_id,
                patch_id,
            } => write!(
                f,
                "approval {approval_id} for patch {patch_id} is approved without decision metadata"
            ),
            Self::WalAppendFailed { patch_id, reason } => {
                write!(f, "failed to append WAL for patch {patch_id}: {reason}")
            }
            Self::InvalidProposal { reason } => {
                write!(f, "invalid wiki patch proposal: {reason}")
            }
        }
    }
}

impl std::error::Error for WikiPatchError {}

fn validate_proposal_shape(proposal: &WikiPatchProposal) -> Result<(), WikiPatchError> {
    let page_id = proposal.page.page_id();
    if page_id.is_empty() {
        return Err(invalid_proposal("page_id must not be empty"));
    }

    let claim_ids = proposal
        .claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    if claim_ids.len() != proposal.claims.len() {
        return Err(invalid_proposal("claim ids must be unique"));
    }

    let citation_ids = proposal
        .citations
        .iter()
        .map(|citation| citation.citation_id.as_str())
        .collect::<BTreeSet<_>>();
    if citation_ids.len() != proposal.citations.len() {
        return Err(invalid_proposal("citation ids must be unique"));
    }

    let citations_by_id = proposal
        .citations
        .iter()
        .map(|citation| (citation.citation_id.as_str(), citation))
        .collect::<BTreeMap<_, _>>();

    for claim in &proposal.claims {
        if claim.page_id != page_id {
            return Err(invalid_proposal(format!(
                "claim {} targets page {}, expected {page_id}",
                claim.claim_id, claim.page_id
            )));
        }

        if claim.citation_ids.is_empty() {
            return Err(invalid_proposal(format!(
                "claim {} must declare at least one citation",
                claim.claim_id
            )));
        }

        for citation_id in &claim.citation_ids {
            let Some(citation) = citations_by_id.get(citation_id.as_str()) else {
                return Err(invalid_proposal(format!(
                    "claim {} references missing citation {citation_id}",
                    claim.claim_id
                )));
            };

            if citation.claim_id != claim.claim_id {
                return Err(invalid_proposal(format!(
                    "claim {} references citation {citation_id} owned by claim {}",
                    claim.claim_id, citation.claim_id
                )));
            }
        }
    }

    for citation in &proposal.citations {
        if !claim_ids.contains(citation.claim_id.as_str()) {
            return Err(invalid_proposal(format!(
                "citation {} targets missing claim {}",
                citation.citation_id, citation.claim_id
            )));
        }
        let claim = proposal
            .claims
            .iter()
            .find(|claim| claim.claim_id == citation.claim_id)
            .expect("claim id was checked above");
        if !claim.citation_ids.contains(&citation.citation_id) {
            return Err(invalid_proposal(format!(
                "citation {} is not declared by claim {}",
                citation.citation_id, citation.claim_id
            )));
        }
    }

    Ok(())
}

fn validate_approval<'a>(
    proposal: &WikiPatchProposal,
    approval: Option<&'a ApprovalRequest>,
) -> Result<&'a ApprovalRequest, WikiPatchError> {
    let Some(approval) = approval else {
        return Err(WikiPatchError::ApprovalRequired {
            patch_id: proposal.patch_id.clone(),
        });
    };

    if approval.patch_id != proposal.patch_id {
        return Err(WikiPatchError::ApprovalPatchMismatch {
            approval_id: approval.approval_id.clone(),
            expected_patch_id: proposal.patch_id.clone(),
            actual_patch_id: approval.patch_id.clone(),
        });
    }

    if approval.workspace_id != proposal.workspace_id {
        return Err(WikiPatchError::ApprovalScopeMismatch {
            approval_id: approval.approval_id.clone(),
            field: "workspace_id",
            expected: proposal.workspace_id.clone(),
            actual: approval.workspace_id.clone(),
        });
    }

    if approval.requested_by != proposal.actor_id {
        return Err(WikiPatchError::ApprovalScopeMismatch {
            approval_id: approval.approval_id.clone(),
            field: "requested_by",
            expected: proposal.actor_id.clone(),
            actual: approval.requested_by.clone(),
        });
    }

    if approval.status != ApprovalStatus::Approved {
        return Err(WikiPatchError::ApprovalNotGranted {
            approval_id: approval.approval_id.clone(),
            patch_id: approval.patch_id.clone(),
            status: approval.status,
        });
    }

    if approval.decided_by.as_deref().is_none_or(str::is_empty) || approval.decided_at.is_none() {
        return Err(WikiPatchError::ApprovalDecisionMissing {
            approval_id: approval.approval_id.clone(),
            patch_id: approval.patch_id.clone(),
        });
    }

    Ok(approval)
}

fn validate_citations(
    proposal: &WikiPatchProposal,
    sources: &[SourceManifest],
    frames: &[ParsedFrame],
) -> Result<(), WikiPatchError> {
    for citation in &proposal.citations {
        validate_citation(citation, proposal, sources, frames)?;
    }

    Ok(())
}

fn validate_citation(
    citation: &Citation,
    proposal: &WikiPatchProposal,
    sources: &[SourceManifest],
    frames: &[ParsedFrame],
) -> Result<(), WikiPatchError> {
    if citation.byte_range.start >= citation.byte_range.end {
        return invalid_citation(citation, "byte range must be non-empty");
    }

    let Some(source) = sources
        .iter()
        .find(|source| source.source_id == citation.source_id)
    else {
        return invalid_citation(citation, "source does not exist");
    };

    if source.workspace_id != proposal.workspace_id {
        return invalid_citation(citation, "source belongs to another workspace");
    }

    if source.visibility == SourceVisibility::Tombstoned || source.tombstoned_at.is_some() {
        return Err(WikiPatchError::TombstonedSource {
            citation_id: citation.citation_id.clone(),
            source_id: citation.source_id.clone(),
        });
    }

    if source.visibility != SourceVisibility::Visible {
        return invalid_citation(citation, "source is not visible to new citations");
    }

    if source.permission_scope != "capability:file.read:source.ingest" {
        return invalid_citation(
            citation,
            "source permission scope is not valid for source.ingest",
        );
    }

    if citation.byte_range.end as u64 > source.size {
        return invalid_citation(citation, "byte range exceeds source size");
    }

    if let Some(frame_id) = &citation.frame_id {
        let Some(frame) = frames
            .iter()
            .find(|frame| frame.frame_id == *frame_id && frame.source_id == citation.source_id)
        else {
            return invalid_citation(citation, "frame does not exist");
        };

        if frame.source_hash != source.raw_content_hash {
            return invalid_citation(citation, "frame source hash does not match source manifest");
        }

        if citation.byte_range.start < frame.byte_range.start
            || citation.byte_range.end > frame.byte_range.end
        {
            return invalid_citation(citation, "byte range exceeds frame range");
        }
    }

    Ok(())
}

fn invalid_citation(citation: &Citation, reason: impl Into<String>) -> Result<(), WikiPatchError> {
    Err(WikiPatchError::InvalidCitation {
        citation_id: citation.citation_id.clone(),
        reason: reason.into(),
    })
}

fn invalid_proposal(reason: impl Into<String>) -> WikiPatchError {
    WikiPatchError::InvalidProposal {
        reason: reason.into(),
    }
}
