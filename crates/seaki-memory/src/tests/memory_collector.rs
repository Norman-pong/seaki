use crate::memory_collector::MemoryCollector;
use crate::memory_item::{MemoryKind, MemoryOrigin, MemoryStatus};
use seaki_index::IndexScope;

fn scope() -> IndexScope {
    IndexScope::new("workspace-a", "account-a")
}

#[test]
fn collector_extracts_preferences_from_session() {
    let collector = MemoryCollector::new();
    let summary = "User prefers dark mode. The team convention is to use 2 spaces. \
                   We must always write tests.";
    let items = collector.extract_from_session(summary, &scope(), "sess-1");

    assert!(!items.is_empty());
    assert!(items
        .iter()
        .any(|i| i.content.to_lowercase().contains("prefer")));
    assert!(items
        .iter()
        .any(|i| i.content.to_lowercase().contains("convention")));
    assert!(items
        .iter()
        .any(|i| i.content.to_lowercase().contains("must")));

    for item in &items {
        assert_eq!(item.kind, MemoryKind::UserPreference);
        assert_eq!(item.status, MemoryStatus::Proposed);
        assert_eq!(item.provenance.origin, MemoryOrigin::SessionHistory);
        assert_eq!(item.provenance.session_id.as_deref(), Some("sess-1"));
    }
}

#[test]
fn collector_extracts_rules_from_wiki_patch() {
    let collector = MemoryCollector::new();
    let patch = r#"diff --git a/docs/rules.md b/docs/rules.md
--- a/docs/rules.md
+++ b/docs/rules.md
@@ -1,3 +1,4 @@
 All contributors must sign the CLA.
+We should always run tests before merging.
+# Code must never contain secrets.
"#;
    let items = collector.extract_from_wiki_patch(patch, &scope(), "abc123");

    assert!(!items.is_empty());
    assert!(items
        .iter()
        .any(|i| i.content.contains("should always run tests")));
    assert!(items
        .iter()
        .any(|i| i.content.contains("must never contain secrets")));

    for item in &items {
        assert_eq!(item.kind, MemoryKind::ProjectConvention);
        assert_eq!(item.provenance.origin, MemoryOrigin::WikiPatch);
        assert_eq!(item.provenance.wiki_patch_hash.as_deref(), Some("abc123"));
    }
}

#[test]
fn collector_extracts_safety_from_approval() {
    let collector = MemoryCollector::new();
    let decision = "The PR introduces a security risk. We need to protect user data. \
                    This is a safety concern.";
    let items = collector.extract_from_approval(decision, &scope(), "actor-1");

    assert!(!items.is_empty());
    assert!(items
        .iter()
        .any(|i| i.content.to_lowercase().contains("security")));
    assert!(items
        .iter()
        .any(|i| i.content.to_lowercase().contains("protect")));

    for item in &items {
        assert_eq!(item.kind, MemoryKind::SafetyRule);
        assert_eq!(item.provenance.origin, MemoryOrigin::ApprovalDecision);
    }
}

#[test]
fn collector_returns_empty_for_plain_text() {
    let collector = MemoryCollector::new();
    let plain = "This is just a normal conversation about the weather and lunch plans.";

    let session_items = collector.extract_from_session(plain, &scope(), "sess-1");
    assert!(session_items.is_empty());

    let patch_items = collector.extract_from_wiki_patch(plain, &scope(), "hash-1");
    assert!(patch_items.is_empty());

    let approval_items = collector.extract_from_approval(plain, &scope(), "actor-1");
    assert!(approval_items.is_empty());
}
