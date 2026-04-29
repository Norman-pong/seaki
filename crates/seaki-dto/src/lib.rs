use std::fmt::Write;

pub const SCHEMA_VERSION: u32 = 1;

pub const INDEX_STATUS_STATE_VARIANTS: &[&str] = &["idle", "indexing", "fresh", "stale", "error"];

pub const DAEMON_CONNECTION_STATUS_VARIANTS: &[&str] = &[
    "daemon.connecting",
    "daemon.ready",
    "daemon.unavailable",
    "daemon.error",
];

pub const IMPORT_STAGE_VARIANTS: &[&str] = &[
    "approval_pending",
    "capability_denied",
    "committed",
    "denied",
    "failed",
    "grant_requested",
    "granted",
    "index_stale",
    "indexed",
    "parse_running",
    "parsed",
    "partial",
    "patch_proposed",
    "raw_committed",
    "selected",
];

pub const APPROVAL_STATUS_VARIANTS: &[&str] = &[
    "pending",
    "approved",
    "applying",
    "committed",
    "rejected",
    "expired",
    "conflict",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStage {
    ApprovalPending,
    CapabilityDenied,
    Committed,
    Denied,
    Failed,
    GrantRequested,
    Granted,
    IndexStale,
    Indexed,
    ParseRunning,
    Parsed,
    Partial,
    PatchProposed,
    RawCommitted,
    Selected,
}

impl ImportStage {
    pub const ALL: [Self; 15] = [
        Self::ApprovalPending,
        Self::CapabilityDenied,
        Self::Committed,
        Self::Denied,
        Self::Failed,
        Self::GrantRequested,
        Self::Granted,
        Self::IndexStale,
        Self::Indexed,
        Self::ParseRunning,
        Self::Parsed,
        Self::Partial,
        Self::PatchProposed,
        Self::RawCommitted,
        Self::Selected,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApprovalPending => "approval_pending",
            Self::CapabilityDenied => "capability_denied",
            Self::Committed => "committed",
            Self::Denied => "denied",
            Self::Failed => "failed",
            Self::GrantRequested => "grant_requested",
            Self::Granted => "granted",
            Self::IndexStale => "index_stale",
            Self::Indexed => "indexed",
            Self::ParseRunning => "parse_running",
            Self::Parsed => "parsed",
            Self::Partial => "partial",
            Self::PatchProposed => "patch_proposed",
            Self::RawCommitted => "raw_committed",
            Self::Selected => "selected",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Parsed
                | Self::Partial
                | Self::Failed
                | Self::Committed
                | Self::Denied
                | Self::Indexed
                | Self::IndexStale
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Applying,
    Committed,
    Rejected,
    Expired,
    Conflict,
}

impl ApprovalStatus {
    pub const ALL: [Self; 7] = [
        Self::Pending,
        Self::Approved,
        Self::Applying,
        Self::Committed,
        Self::Rejected,
        Self::Expired,
        Self::Conflict,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Applying => "applying",
            Self::Committed => "committed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Conflict => "conflict",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::Rejected | Self::Expired | Self::Conflict
        )
    }
}

#[derive(Debug)]
pub struct StringUnion {
    pub name: &'static str,
    pub variants: &'static [&'static str],
}

#[derive(Debug)]
pub struct Interface {
    pub name: &'static str,
    pub generic: Option<&'static str>,
    pub fields: &'static [Field],
}

#[derive(Debug)]
pub struct Field {
    pub name: &'static str,
    pub ts_type: &'static str,
    pub optional: bool,
}

impl Field {
    #[must_use]
    pub const fn required(name: &'static str, ts_type: &'static str) -> Self {
        Self {
            name,
            ts_type,
            optional: false,
        }
    }

    #[must_use]
    pub const fn optional(name: &'static str, ts_type: &'static str) -> Self {
        Self {
            name,
            ts_type,
            optional: true,
        }
    }
}

#[derive(Debug)]
pub struct MethodName {
    pub key: &'static str,
    pub value: &'static str,
}

pub const STRING_UNIONS: &[StringUnion] = &[
    StringUnion {
        name: "IndexStatusState",
        variants: INDEX_STATUS_STATE_VARIANTS,
    },
    StringUnion {
        name: "DaemonConnectionStatus",
        variants: DAEMON_CONNECTION_STATUS_VARIANTS,
    },
    StringUnion {
        name: "ImportStage",
        variants: IMPORT_STAGE_VARIANTS,
    },
    StringUnion {
        name: "ApprovalStatus",
        variants: APPROVAL_STATUS_VARIANTS,
    },
];

pub const INTERFACES: &[Interface] = &[
    Interface {
        name: "IndexStatusDTO",
        generic: None,
        fields: &[
            Field::required("state", "IndexStatusState"),
            Field::required("last_good_revision", "string | null"),
            Field::required("stale_reason", "string | null"),
            Field::required("updated_at", "string | null"),
        ],
    },
    Interface {
        name: "SourceRangeDTO",
        generic: None,
        fields: &[
            Field::required("unit", "\"byte\" | \"line\" | \"page\" | \"anchor\""),
            Field::required("start", "number"),
            Field::required("end", "number"),
            Field::required("label", "string | null"),
        ],
    },
    Interface {
        name: "WorkspaceDTO",
        generic: None,
        fields: &[
            Field::required("workspace_id", "string"),
            Field::required("root_uri", "string"),
            Field::required(
                "state",
                "\"empty\" | \"ready\" | \"degraded\" | \"audit_readonly\"",
            ),
            Field::required("current_revision", "string"),
            Field::required("audit_head", "string"),
            Field::required("index_status", "IndexStatusDTO"),
        ],
    },
    Interface {
        name: "UserSelectedFileDTO",
        generic: None,
        fields: &[
            Field::required("selection_id", "string"),
            Field::required("display_name", "string"),
            Field::required("platform", "\"electron\""),
            Field::required("opaque_file_ref", "string"),
            Field::required("declared_size", "number"),
            Field::required("declared_mime", "string"),
        ],
    },
    Interface {
        name: "CapabilityGrantRequestDTO",
        generic: None,
        fields: &[
            Field::required("operation", "string"),
            Field::required("target", "string"),
            Field::required("ttl", "number"),
            Field::required("uses", "number"),
            Field::required("reason", "string"),
            Field::required("risk_summary", "string"),
        ],
    },
    Interface {
        name: "SourceManifestDTO",
        generic: None,
        fields: &[
            Field::required("source_id", "string"),
            Field::required("origin_display", "string"),
            Field::required("mime", "string"),
            Field::required("size", "number"),
            Field::required("parse_status", "ImportStage"),
            Field::required("permission_scope", "string"),
        ],
    },
    Interface {
        name: "CitationRefDTO",
        generic: None,
        fields: &[
            Field::required("citation_id", "string"),
            Field::required("source_id", "string"),
            Field::required("range", "SourceRangeDTO"),
            Field::required("wiki_page_id", "string"),
            Field::required("claim_id", "string"),
            Field::required("degraded_reason", "string | null"),
        ],
    },
    Interface {
        name: "SourceCardDTO",
        generic: None,
        fields: &[
            Field::required("source_id", "string"),
            Field::required("title", "string"),
            Field::required("origin_display", "string"),
            Field::required("range", "SourceRangeDTO"),
            Field::required("summary", "string"),
            Field::required(
                "visibility",
                "\"visible\" | \"restricted\" | \"tombstoned\"",
            ),
            Field::required("citation_refs", "readonly CitationRefDTO[]"),
        ],
    },
    Interface {
        name: "AnnotationDTO",
        generic: None,
        fields: &[
            Field::required("annotation_id", "string"),
            Field::required("source_id", "string"),
            Field::required("range", "SourceRangeDTO"),
            Field::required("note", "string"),
            Field::required("created_by", "string"),
            Field::required("created_at", "string"),
            Field::required("supersede_of", "string | null"),
            Field::required("conflict_status", "string | null"),
        ],
    },
    Interface {
        name: "RiskSummaryDTO",
        generic: None,
        fields: &[
            Field::required("level", "\"low\" | \"medium\" | \"high\" | \"critical\""),
            Field::required("summary", "string"),
            Field::required("factors", "readonly string[]"),
            Field::required("requires_manual_approval", "boolean"),
        ],
    },
    Interface {
        name: "PatchDiffDTO",
        generic: None,
        fields: &[
            Field::required("format", "\"unified\" | \"structured\""),
            Field::required("text", "string"),
            Field::required("affected_paths", "readonly string[]"),
            Field::required("added_lines", "number"),
            Field::required("removed_lines", "number"),
        ],
    },
    Interface {
        name: "CitationEvidenceDTO",
        generic: None,
        fields: &[
            Field::required("citation_id", "string"),
            Field::required("source_id", "string"),
            Field::required("source_title", "string"),
            Field::required("range", "SourceRangeDTO"),
            Field::required("cited_ranges", "readonly SourceRangeDTO[]"),
            Field::required("excerpt", "string"),
            Field::required(
                "visibility",
                "\"visible\" | \"restricted\" | \"tombstoned\"",
            ),
            Field::required("degraded_reason", "string | null"),
        ],
    },
    Interface {
        name: "CitationValidationDTO",
        generic: None,
        fields: &[
            Field::required("citation_id", "string"),
            Field::optional("claim_id", "string | null"),
            Field::required("state", "\"valid\" | \"invalid\" | \"degraded\""),
            Field::required("reason", "string | null"),
            Field::optional("evidence", "readonly CitationEvidenceDTO[]"),
            Field::optional("cited_ranges", "readonly SourceRangeDTO[]"),
            Field::optional("taint_flags", "readonly string[]"),
            Field::optional("security_flags", "readonly string[]"),
        ],
    },
    Interface {
        name: "ClaimReviewDTO",
        generic: None,
        fields: &[
            Field::required("claim_id", "string"),
            Field::required("page_id", "string"),
            Field::required("text", "string"),
            Field::required("citation_ids", "readonly string[]"),
            Field::required("citation_validation", "readonly CitationValidationDTO[]"),
            Field::required("risk_summary", "RiskSummaryDTO"),
            Field::required("taint_flags", "readonly string[]"),
            Field::required("security_flags", "readonly string[]"),
        ],
    },
    Interface {
        name: "WikiPatchProposalDTO",
        generic: None,
        fields: &[
            Field::required("patch_id", "string"),
            Field::required("base_revision", "string"),
            Field::required("diff", "string | PatchDiffDTO"),
            Field::required("claim_ids", "readonly string[]"),
            Field::optional("claims", "readonly ClaimReviewDTO[]"),
            Field::required("citation_validation", "readonly CitationValidationDTO[]"),
            Field::required("risk_summary", "string | RiskSummaryDTO"),
            Field::optional("taint_flags", "readonly string[]"),
            Field::optional("security_flags", "readonly string[]"),
        ],
    },
    Interface {
        name: "ApprovalClaimDecisionDTO",
        generic: None,
        fields: &[
            Field::required("claim_id", "string"),
            Field::required("decision", "\"approve\" | \"reject\""),
            Field::required("reason", "string | null"),
            Field::required("decided_by", "string | null"),
            Field::required("decided_at", "string | null"),
        ],
    },
    Interface {
        name: "ApprovalRequestDTO",
        generic: None,
        fields: &[
            Field::required("approval_id", "string"),
            Field::required("patch_id", "string"),
            Field::optional("status", "ApprovalStatus"),
            Field::required("required_by", "string"),
            Field::required("expires_at", "string"),
            Field::required(
                "policy_decision",
                "\"allow\" | \"deny\" | \"requires_approval\"",
            ),
            Field::optional("proposal", "WikiPatchProposalDTO"),
            Field::optional("claim_decisions", "readonly ApprovalClaimDecisionDTO[]"),
            Field::optional("rejection_reason", "string | null"),
            Field::optional("wal_entry_id", "string | null"),
            Field::optional("audit_id", "string | null"),
        ],
    },
    Interface {
        name: "ApprovalReviewDTO",
        generic: None,
        fields: &[
            Field::required("request", "ApprovalRequestDTO"),
            Field::required("proposal", "WikiPatchProposalDTO"),
        ],
    },
    Interface {
        name: "ApprovalDecisionResultDTO",
        generic: None,
        fields: &[
            Field::required("approval_id", "string"),
            Field::required("patch_id", "string"),
            Field::required("status", "ApprovalStatus"),
            Field::required("claim_decisions", "readonly ApprovalClaimDecisionDTO[]"),
            Field::required("rejection_reason", "string | null"),
            Field::required("wal_entry_id", "string | null"),
            Field::required("audit_id", "string | null"),
            Field::required("transaction_id", "string | null"),
            Field::required("committed_revision", "string | null"),
            Field::required("denied_reason", "string | null"),
        ],
    },
    Interface {
        name: "SearchResultDTO",
        generic: None,
        fields: &[
            Field::required("result_id", "string"),
            Field::required("kind", "\"wiki_page\" | \"claim\" | \"source\""),
            Field::required("title", "string"),
            Field::required("snippet", "string | null"),
            Field::required("citation_refs", "readonly CitationRefDTO[]"),
            Field::required("index_status", "IndexStatusDTO"),
        ],
    },
    Interface {
        name: "AnswerDTO",
        generic: None,
        fields: &[
            Field::required("answer_id", "string"),
            Field::required("text", "string"),
            Field::required("citation_refs", "readonly CitationRefDTO[]"),
            Field::required("status", "\"composed\" | \"degraded\" | \"no_access\""),
        ],
    },
    Interface {
        name: "CitationResolveResultDTO",
        generic: None,
        fields: &[
            Field::required("citation_id", "string"),
            Field::required("source_id", "string"),
            Field::required("range", "SourceRangeDTO"),
            Field::required("wiki_page_id", "string"),
            Field::required("claim_id", "string"),
            Field::required(
                "preview_target",
                "\"source_range\" | \"wiki_anchor\" | \"none\"",
            ),
            Field::required("degraded_reason", "string | null"),
            Field::optional("source_card", "SourceCardDTO | null"),
        ],
    },
    Interface {
        name: "ChannelAnswerDTO",
        generic: None,
        fields: &[
            Field::required("message_id", "string"),
            Field::required("thread_id", "string"),
            Field::required("audit_id", "string"),
            Field::required("transaction_id", "string"),
            Field::required("citation_ids", "readonly string[]"),
        ],
    },
    Interface {
        name: "OutboxItemDTO",
        generic: None,
        fields: &[
            Field::required("outbox_id", "string"),
            Field::required("transaction_id", "string"),
            Field::required("state", "\"pending\" | \"sent\" | \"failed\""),
            Field::required("provider_idempotency_key", "string"),
            Field::required("attempt_count", "number"),
            Field::required("next_attempt_at", "string | null"),
        ],
    },
    Interface {
        name: "ChannelEvent",
        generic: None,
        fields: &[
            Field::required("event_id", "string"),
            Field::required("event_type", "string"),
            Field::required("provider_tenant_id", "string"),
            Field::required("channel_binding_id", "string"),
            Field::required("provider_user_id", "string"),
            Field::required("payload", "unknown"),
            Field::required("timestamp", "string"),
        ],
    },
    Interface {
        name: "ChannelActionGrant",
        generic: None,
        fields: &[
            Field::required("grant_id", "string"),
            Field::required("scope", "string"),
            Field::required("audience", "string"),
            Field::required("ttl", "number"),
            Field::required("uses_remaining", "number"),
            Field::required("idempotency_key", "string"),
            Field::required("allowed_actions", "readonly string[]"),
            Field::required("provenance", "Provenance"),
        ],
    },
    Interface {
        name: "ChannelResourceGrant",
        generic: None,
        fields: &[
            Field::required("grant_id", "string"),
            Field::required("scope", "string"),
            Field::required("file_key", "string"),
            Field::required("version", "string"),
            Field::required("uses_remaining", "number"),
            Field::required("issued_at", "string"),
            Field::required("expires_at", "string"),
        ],
    },
    Interface {
        name: "Provenance",
        generic: None,
        fields: &[
            Field::required("transaction_id", "string"),
            Field::required("source_id", "string"),
            Field::required("citation_ids", "readonly string[]"),
            Field::required("thread_scope", "string"),
            Field::required("audit_id", "string"),
        ],
    },
    Interface {
        name: "PipeCommandSummaryDTO",
        generic: None,
        fields: &[
            Field::required("command_id", "string"),
            Field::required("description", "string"),
            Field::required("side_effect_level", "string"),
        ],
    },
    Interface {
        name: "PipelineDryRunInputDTO",
        generic: None,
        fields: &[
            Field::required("pipeline_id", "string"),
            Field::required("steps", "readonly PipelineStepInputDTO[]"),
            Field::required("initial_input", "unknown"),
        ],
    },
    Interface {
        name: "PipelineStepInputDTO",
        generic: None,
        fields: &[
            Field::required("step_id", "string"),
            Field::required("command_id", "string"),
            Field::optional("input_binding", "string | null"),
            Field::optional("failure_policy", "string | null"),
        ],
    },
    Interface {
        name: "DryRunResultDTO",
        generic: None,
        fields: &[
            Field::required("events", "readonly DryRunEventDTO[]"),
            Field::required("expected_read_ranges", "readonly string[]"),
            Field::required("expected_permissions", "readonly string[]"),
            Field::required("expected_frame_count", "number"),
            Field::optional("proposal_artifact", "PatchProposalArtifactDTO | null"),
        ],
    },
    Interface {
        name: "DryRunEventDTO",
        generic: None,
        fields: &[
            Field::required("event_type", "string"),
            Field::required("step_id", "string | null"),
            Field::optional("payload", "unknown"),
        ],
    },
    Interface {
        name: "PatchProposalArtifactDTO",
        generic: None,
        fields: &[
            Field::required("patch_id", "string"),
            Field::required("base_revision", "string"),
            Field::required("diff", "string"),
            Field::required("claim_ids", "readonly string[]"),
        ],
    },
    Interface {
        name: "MemoryNoteDTO",
        generic: None,
        fields: &[
            Field::required("note_id", "string"),
            Field::required("title", "string"),
            Field::required("content", "string"),
            Field::required("created_at", "string"),
            Field::required("updated_at", "string"),
            Field::required("status", "string"),
        ],
    },
    Interface {
        name: "MemoryProposeInputDTO",
        generic: None,
        fields: &[
            Field::required("title", "string"),
            Field::required("content", "string"),
            Field::required("workspace_id", "string"),
        ],
    },
    Interface {
        name: "SessionSearchCandidateDTO",
        generic: None,
        fields: &[
            Field::required("session_id", "string"),
            Field::required("summary", "string"),
            Field::required("redacted_at", "string"),
        ],
    },
    Interface {
        name: "ChannelOutboxQueryResultDTO",
        generic: None,
        fields: &[
            Field::required("items", "readonly OutboxItemDTO[]"),
            Field::required("total_pending", "number"),
            Field::required("total_unknown", "number"),
        ],
    },
    Interface {
        name: "FrontendEventEnvelope",
        generic: Some("<TPayload = unknown>"),
        fields: &[
            Field::required("event_id", "string"),
            Field::required("schema_version", "typeof SCHEMA_VERSION"),
            Field::required("payload_schema_hash", "string"),
            Field::required("seq", "number"),
            Field::required("workspace_id", "string"),
            Field::required("actor_id", "string"),
            Field::required("scope", "string"),
            Field::optional("task_id", "string"),
            Field::optional("transaction_id", "string"),
            Field::required("correlation_id", "string"),
            Field::optional("causation_id", "string"),
            Field::required("revision", "string"),
            Field::required("occurred_at", "string"),
            Field::required("replayable", "boolean"),
            Field::required("idempotency_key", "string"),
            Field::required("type", "string"),
            Field::required("payload", "TPayload"),
        ],
    },
];

pub const M0_DOMAIN_USE_CASE_METHODS: &[MethodName] = &[
    MethodName {
        key: "WORKSPACE_INIT",
        value: "workspace.init",
    },
    MethodName {
        key: "FILES_PREPARE_USER_SELECTED",
        value: "files.prepareUserSelected",
    },
    MethodName {
        key: "SOURCE_INGEST_SELECTED_FILE",
        value: "source.ingestSelectedFile",
    },
    MethodName {
        key: "APPROVAL_REVIEW_PATCH",
        value: "approval.reviewPatch",
    },
    MethodName {
        key: "APPROVAL_DECIDE",
        value: "approval.decide",
    },
    MethodName {
        key: "WIKI_READ_PAGE",
        value: "wiki.readPage",
    },
    MethodName {
        key: "SEARCH_QUERY",
        value: "search.query",
    },
    MethodName {
        key: "CITATION_RESOLVE",
        value: "citation.resolve",
    },
    MethodName {
        key: "PIPE_LIST",
        value: "pipeline.list",
    },
    MethodName {
        key: "PIPE_INSPECT",
        value: "pipeline.inspect",
    },
    MethodName {
        key: "PIPE_DRY_RUN",
        value: "pipeline.dryRun",
    },
    MethodName {
        key: "MEMORY_PROPOSE",
        value: "memory.propose",
    },
    MethodName {
        key: "MEMORY_SEARCH",
        value: "memory.searchNotes",
    },
    MethodName {
        key: "SESSION_SEARCH",
        value: "memory.sessionSearch",
    },
    MethodName {
        key: "CHANNEL_OUTBOX_QUERY",
        value: "channel.outbox.query",
    },
];

#[must_use]
pub fn canonical_schema() -> String {
    let mut schema = String::new();
    writeln!(schema, "schema_version:{SCHEMA_VERSION}").unwrap();
    schema.push_str("[string_unions]\n");
    for string_union in STRING_UNIONS {
        schema.push_str(string_union.name);
        schema.push('=');
        schema.push_str(&string_union.variants.join("|"));
        schema.push('\n');
    }

    schema.push_str("[interfaces]\n");
    for interface in INTERFACES {
        schema.push_str(interface.name);
        if let Some(generic) = interface.generic {
            schema.push_str(generic);
        }
        schema.push('\n');
        for field in interface.fields {
            schema.push_str(field.name);
            if field.optional {
                schema.push('?');
            }
            schema.push(':');
            schema.push_str(field.ts_type);
            schema.push('\n');
        }
    }

    schema.push_str("[m0_domain_use_case_methods]\n");
    for method in M0_DOMAIN_USE_CASE_METHODS {
        schema.push_str(method.key);
        schema.push('=');
        schema.push_str(method.value);
        schema.push('\n');
    }

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_stage_variants_follow_enum_order() {
        let from_enum = ImportStage::ALL
            .iter()
            .map(|stage| stage.as_str())
            .collect::<Vec<_>>();

        assert_eq!(from_enum, IMPORT_STAGE_VARIANTS);
    }

    #[test]
    fn approval_status_variants_follow_enum_order() {
        let from_enum = ApprovalStatus::ALL
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>();

        assert_eq!(from_enum, APPROVAL_STATUS_VARIANTS);
        assert!(ApprovalStatus::Committed.is_terminal());
        assert!(!ApprovalStatus::Pending.is_terminal());
    }

    #[test]
    fn frontend_minimum_dtos_are_in_schema() {
        let names = INTERFACES
            .iter()
            .map(|interface| interface.name)
            .collect::<Vec<_>>();

        for required in [
            "CapabilityGrantRequestDTO",
            "SourceManifestDTO",
            "SourceCardDTO",
            "AnnotationDTO",
            "PatchDiffDTO",
            "ClaimReviewDTO",
            "CitationEvidenceDTO",
            "WikiPatchProposalDTO",
            "ApprovalRequestDTO",
            "ApprovalReviewDTO",
            "ApprovalDecisionResultDTO",
            "SearchResultDTO",
            "CitationRefDTO",
            "AnswerDTO",
            "CitationResolveResultDTO",
            "ChannelAnswerDTO",
            "OutboxItemDTO",
            "ChannelEvent",
            "ChannelActionGrant",
            "ChannelResourceGrant",
            "Provenance",
        ] {
            assert!(
                names.contains(&required),
                "missing DTO contract: {required}"
            );
        }
    }
}
