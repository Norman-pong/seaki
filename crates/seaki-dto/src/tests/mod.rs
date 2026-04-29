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
