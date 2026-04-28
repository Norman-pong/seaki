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
    pub const fn required(name: &'static str, ts_type: &'static str) -> Self {
        Self {
            name,
            ts_type,
            optional: false,
        }
    }

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
        name: "CitationValidationDTO",
        generic: None,
        fields: &[
            Field::required("citation_id", "string"),
            Field::required("state", "\"valid\" | \"invalid\" | \"degraded\""),
            Field::required("reason", "string | null"),
        ],
    },
    Interface {
        name: "WikiPatchProposalDTO",
        generic: None,
        fields: &[
            Field::required("patch_id", "string"),
            Field::required("base_revision", "string"),
            Field::required("diff", "string"),
            Field::required("claim_ids", "readonly string[]"),
            Field::required("citation_validation", "readonly CitationValidationDTO[]"),
            Field::required("risk_summary", "string"),
        ],
    },
    Interface {
        name: "ApprovalRequestDTO",
        generic: None,
        fields: &[
            Field::required("approval_id", "string"),
            Field::required("patch_id", "string"),
            Field::required("required_by", "string"),
            Field::required("expires_at", "string"),
            Field::required(
                "policy_decision",
                "\"allow\" | \"deny\" | \"requires_approval\"",
            ),
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
];

pub fn canonical_schema() -> String {
    let mut schema = String::new();
    schema.push_str(&format!("schema_version:{}\n", SCHEMA_VERSION));
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
            "WikiPatchProposalDTO",
            "ApprovalRequestDTO",
            "SearchResultDTO",
            "CitationRefDTO",
            "ChannelAnswerDTO",
            "OutboxItemDTO",
        ] {
            assert!(
                names.contains(&required),
                "missing DTO contract: {required}"
            );
        }
    }
}
