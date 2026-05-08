use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use crate::runbook_index::{RunbookEntry, RunbookIndex};
use seaki_index::IndexScope;

fn make_rule_item(id: &str, content: &str) -> MemoryItem {
    MemoryItem {
        memory_id: id.to_string(),
        kind: MemoryKind::SafetyRule,
        scope: IndexScope::new("ws", "ac"),
        content: content.to_string(),
        source_citation: None,
        proposed_at: 1000,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status: MemoryStatus::Active,
        trust_level: TrustLevel::Confirmed,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::WikiPatch,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

fn _make_workflow_item(id: &str, content: &str) -> MemoryItem {
    MemoryItem {
        memory_id: id.to_string(),
        kind: MemoryKind::WorkflowPattern,
        scope: IndexScope::new("ws", "ac"),
        content: content.to_string(),
        source_citation: None,
        proposed_at: 1000,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status: MemoryStatus::Active,
        trust_level: TrustLevel::Confirmed,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::WikiPatch,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

#[test]
fn runbook_index_insert_and_search() {
    let mut index = RunbookIndex::new();
    let entry = RunbookEntry {
        entry_id: "runbook_0".to_string(),
        topic_id: "topic_auth".to_string(),
        title: "Authentication Setup".to_string(),
        description: "How to configure OAuth authentication".to_string(),
        pipeline_template_id: Some("tpl-auth".to_string()),
        required_capabilities: vec!["oauth".to_string()],
        source_memory_ids: vec!["mem-1".to_string()],
        created_at: 1000,
    };
    index.insert(entry);

    assert_eq!(index.len(), 1);
    let results = index.search("OAuth");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Authentication Setup");
}

#[test]
fn runbook_index_by_topic() {
    let mut index = RunbookIndex::new();
    index.insert(RunbookEntry {
        entry_id: "runbook_0".to_string(),
        topic_id: "topic_auth".to_string(),
        title: "Auth Setup".to_string(),
        description: "OAuth setup".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec![],
        source_memory_ids: vec![],
        created_at: 1000,
    });
    index.insert(RunbookEntry {
        entry_id: "runbook_1".to_string(),
        topic_id: "topic_auth".to_string(),
        title: "Auth Cleanup".to_string(),
        description: "OAuth cleanup".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec![],
        source_memory_ids: vec![],
        created_at: 1000,
    });
    index.insert(RunbookEntry {
        entry_id: "runbook_2".to_string(),
        topic_id: "topic_deploy".to_string(),
        title: "Deploy".to_string(),
        description: "Deployment".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec![],
        source_memory_ids: vec![],
        created_at: 1000,
    });

    let auth_entries = index.by_topic("topic_auth");
    assert_eq!(auth_entries.len(), 2);
}

#[test]
fn runbook_index_by_capability() {
    let mut index = RunbookIndex::new();
    index.insert(RunbookEntry {
        entry_id: "runbook_0".to_string(),
        topic_id: "topic_auth".to_string(),
        title: "Auth Setup".to_string(),
        description: "OAuth setup".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec!["oauth".to_string(), "admin".to_string()],
        source_memory_ids: vec![],
        created_at: 1000,
    });
    index.insert(RunbookEntry {
        entry_id: "runbook_1".to_string(),
        topic_id: "topic_deploy".to_string(),
        title: "Deploy".to_string(),
        description: "Deployment".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec!["admin".to_string()],
        source_memory_ids: vec![],
        created_at: 1000,
    });

    let admin_entries = index.by_capability("admin");
    assert_eq!(admin_entries.len(), 2);

    let oauth_entries = index.by_capability("oauth");
    assert_eq!(oauth_entries.len(), 1);
}

#[test]
fn runbook_auto_generate_from_rules() {
    let mut index = RunbookIndex::new();
    let items = [
        make_rule_item("mem-1", "Always validate user input before processing."),
        make_rule_item("mem-2", "Never expose secrets in logs."),
    ];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    index.auto_generate(&refs, 5000);

    assert_eq!(index.len(), 2);
    let results = index.search("validate");
    assert_eq!(results.len(), 1);
}

#[test]
fn runbook_index_search_keyword() {
    let mut index = RunbookIndex::new();
    index.insert(RunbookEntry {
        entry_id: "runbook_0".to_string(),
        topic_id: "topic_a".to_string(),
        title: "Kubernetes Deployment".to_string(),
        description: "Deploy services to Kubernetes cluster".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec![],
        source_memory_ids: vec![],
        created_at: 1000,
    });
    index.insert(RunbookEntry {
        entry_id: "runbook_1".to_string(),
        topic_id: "topic_b".to_string(),
        title: "Database Migration".to_string(),
        description: "Run database schema migrations".to_string(),
        pipeline_template_id: None,
        required_capabilities: vec![],
        source_memory_ids: vec![],
        created_at: 1000,
    });

    let k8s = index.search("kubernetes");
    assert_eq!(k8s.len(), 1);

    let db = index.search("schema");
    assert_eq!(db.len(), 1);

    let none = index.search("nonexistent");
    assert!(none.is_empty());
}

#[test]
fn runbook_auto_generate_skips_non_rule_workflow() {
    let mut index = RunbookIndex::new();
    let items = [
        make_rule_item("mem-1", "Rule one."),
        MemoryItem {
            memory_id: "mem-2".to_string(),
            kind: MemoryKind::DerivedFact,
            scope: IndexScope::new("ws", "ac"),
            content: "Some fact".to_string(),
            source_citation: None,
            proposed_at: 1000,
            confirmed_at: None,
            last_verified_at: None,
            expires_at: None,
            status: MemoryStatus::Active,
            trust_level: TrustLevel::Confirmed,
            confirmed_by: None,
            provenance: MemoryProvenance {
                origin: MemoryOrigin::WikiPatch,
                extraction_method: "test".to_string(),
                session_id: None,
                wiki_patch_hash: None,
            },
        },
    ];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    index.auto_generate(&refs, 5000);

    assert_eq!(index.len(), 1);
}
