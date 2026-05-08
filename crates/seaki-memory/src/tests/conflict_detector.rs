use crate::conflict_detector::{ConflictDetector, ConflictResolution, ConflictType};
use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use seaki_index::IndexScope;

fn scope() -> IndexScope {
    IndexScope::new("workspace-a", "account-a")
}

fn dummy_item(content: &str) -> MemoryItem {
    MemoryItem {
        memory_id: "mem-1".to_string(),
        kind: MemoryKind::DerivedFact,
        scope: scope(),
        content: content.to_string(),
        source_citation: None,
        proposed_at: 100,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status: MemoryStatus::Proposed,
        trust_level: TrustLevel::Unverified,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::SystemInferred,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

#[test]
fn detector_finds_keyword_conflict() {
    let detector = ConflictDetector::new();
    let item = dummy_item("The project uses React and TypeScript.");
    let keywords = vec!["react".to_string(), "typescript".to_string()];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].memory_id, "mem-1");
    assert_eq!(reports[0].conflict_type, ConflictType::KeywordOverlap);
    assert!(reports[0]
        .conflicting_keywords
        .contains(&"react".to_string()));
    assert!(reports[0]
        .conflicting_keywords
        .contains(&"typescript".to_string()));
}

#[test]
fn detector_recommends_downgrade() {
    let detector = ConflictDetector::new();
    let item = dummy_item("We use React.");
    let keywords = vec!["react".to_string()];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].recommendation,
        ConflictResolution::DowngradeToStale
    );
}

#[test]
fn detector_recommends_reject_for_many_conflicts() {
    let detector = ConflictDetector::new();
    let item = dummy_item("Alpha beta gamma delta epsilon.");
    let keywords = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
        "delta".to_string(),
    ];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].recommendation, ConflictResolution::Reject);
}

#[test]
fn detector_ignores_empty_keyword() {
    let detector = ConflictDetector::new();
    let item = dummy_item("The project uses React.");
    let keywords = vec!["".to_string(), "react".to_string()];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].conflicting_keywords, vec!["react"]);
}

#[test]
fn detector_no_false_positive_for_substring() {
    let detector = ConflictDetector::new();
    let item = dummy_item("We should trust the process.");
    let keywords = vec!["rust".to_string()];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert!(reports.is_empty());
}

#[test]
fn detector_no_conflict_for_unrelated() {
    let detector = ConflictDetector::new();
    let item = dummy_item("We use Vue and Python.");
    let keywords = vec!["react".to_string(), "typescript".to_string()];

    let reports = detector.detect_conflicts(&item, &keywords);
    assert!(reports.is_empty());
}
