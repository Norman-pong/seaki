use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

use crate::audit::AuditAction;
use crate::grant::{file_read_grant_scope_hash, snapshot_file, FileReadGrantScope};

#[test]
fn policy_default_shape_keeps_grants_opaque() {
    assert_eq!(CAPABILITY_GRANT_VISIBILITY, "opaque-id-only");
    assert!(PolicyDecision::Allow.permits_side_effect());
    assert!(!PolicyDecision::Deny.permits_side_effect());
    assert!(!PolicyDecision::RequireApproval.permits_side_effect());
}

#[test]
fn workspace_external_path_is_denied_by_default() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("outside.txt", "secret");
    let engine = fixture.engine();

    let evaluation = engine
        .authorize_file_read(&fixture.request(external_file, None))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(evaluation.reason, PolicyReason::PathOutsideWorkspace);
}

#[test]
fn symlink_escape_is_denied() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("outside.txt", "secret");
    let symlink_path = fixture.workspace.path().join("link-outside.txt");
    create_symlink(&external_file, &symlink_path);
    let engine = fixture.engine();

    let evaluation = engine
        .authorize_file_read(&fixture.request(symlink_path, None))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(evaluation.reason, PolicyReason::PathOutsideWorkspace);
}

#[test]
fn workspace_denylist_overrides_allowlist() {
    let fixture = Fixture::new();
    let denied_dir = fixture.workspace.path().join("private");
    fs::create_dir(&denied_dir).expect("create denied dir");
    let denied_file = denied_dir.join("note.md");
    fs::write(&denied_file, "secret").expect("write denied file");
    let policy = WorkspacePathPolicy::try_new(fixture.workspace.path())
        .expect("workspace policy")
        .with_deny_roots([denied_dir])
        .expect("deny roots");
    let engine = PolicyEngine::new(policy);

    let evaluation = engine
        .authorize_file_read(&fixture.request(denied_file, None))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(evaluation.reason, PolicyReason::PathDenied);
}

#[test]
fn workspace_denylist_cannot_be_bypassed_by_grant() {
    let fixture = Fixture::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let denied_dir = fixture.workspace.path().join("private");
    fs::create_dir(&denied_dir).expect("create denied dir");
    let denied_file = denied_dir.join("note.md");
    fs::write(&denied_file, "secret").expect("write denied file");
    let policy = WorkspacePathPolicy::try_new(fixture.workspace.path())
        .expect("workspace policy")
        .with_deny_roots([denied_dir])
        .expect("deny roots");
    let engine = PolicyEngine::with_fixed_now(policy, now);
    engine
        .capability_store()
        .issue_file_read_grant(fixture.grant_input(&denied_file, now))
        .expect("issue grant")
        .expect("approved grant");

    let evaluation = engine
        .authorize_file_read(&fixture.request_at(&denied_file, Some("cap-source"), now))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(evaluation.reason, PolicyReason::PathDenied);
    assert_eq!(
        engine
            .capability_store()
            .uses_remaining("cap-source")
            .expect("uses remaining"),
        Some(1)
    );
}

#[test]
fn default_deny_roots_apply_to_directories_created_after_policy_init() {
    let fixture = Fixture::new();
    let engine = fixture.engine();
    let denied_dir = fixture.workspace.path().join(".seaki");
    fs::create_dir(&denied_dir).expect("create denied dir after policy init");
    let denied_file = denied_dir.join("secret");
    fs::write(&denied_file, "secret").expect("write denied file");

    let evaluation = engine
        .authorize_file_read(&fixture.request(denied_file, None))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(evaluation.reason, PolicyReason::PathDenied);
}

#[test]
fn grant_is_single_use_and_bound_to_audience() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("source.md", "# source");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let engine = fixture.engine_at(now);
    engine
        .capability_store()
        .issue_file_read_grant(fixture.grant_input(&external_file, now))
        .expect("issue grant")
        .expect("approved grant");

    let wrong_audience = engine
        .authorize_file_read(&FileReadPolicyRequest {
            audience: "seaki-other".to_string(),
            capability_id: Some("cap-source".to_string()),
            ..fixture.request_at(&external_file, None, now)
        })
        .expect("wrong audience evaluation");
    assert_eq!(wrong_audience.decision, PolicyDecision::Deny);
    assert_eq!(
        wrong_audience.reason,
        PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::WrongAudience)
    );
    assert_eq!(
        engine
            .capability_store()
            .uses_remaining("cap-source")
            .expect("uses remaining"),
        Some(1)
    );

    let allowed = engine
        .authorize_file_read(&fixture.request_at(&external_file, Some("cap-source"), now))
        .expect("allowed evaluation");
    assert_eq!(allowed.decision, PolicyDecision::Allow);
    assert_eq!(allowed.reason, PolicyReason::CapabilityGrant);
    assert_eq!(
        engine
            .capability_store()
            .uses_remaining("cap-source")
            .expect("uses remaining"),
        Some(0)
    );

    let reused = engine
        .authorize_file_read(&fixture.request_at(&external_file, Some("cap-source"), now))
        .expect("reuse evaluation");
    assert_eq!(reused.decision, PolicyDecision::Deny);
    assert_eq!(
        reused.reason,
        PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::AlreadyUsed)
    );
}

#[test]
fn expired_grant_is_rejected_without_consuming_use() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("source.md", "# source");
    let issued_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let engine = fixture.engine_at(issued_at + Duration::from_secs(61));
    engine
        .capability_store()
        .issue_file_read_grant(fixture.grant_input(&external_file, issued_at))
        .expect("issue grant")
        .expect("approved grant");

    let evaluation = engine
        .authorize_file_read(&fixture.request_at(&external_file, Some("cap-source"), issued_at))
        .expect("expired evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(
        evaluation.reason,
        PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::Expired)
    );
    assert_eq!(
        engine
            .capability_store()
            .uses_remaining("cap-source")
            .expect("uses remaining"),
        Some(1)
    );
}

#[test]
fn denied_approval_cannot_issue_file_read_grant() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("source.md", "# source");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let store = CapabilityStore::new();

    let result = store
        .issue_file_read_grant(FileReadGrantInput {
            approval: fixture.approval(now, ApprovalStatus::Denied),
            ..fixture.grant_input(&external_file, now)
        })
        .expect("issue grant evaluation");

    assert_eq!(result, Err(CapabilityGrantRejection::ApprovalNotApproved));
    assert_eq!(
        store.uses_remaining("cap-source").expect("uses remaining"),
        None
    );
}

#[test]
fn approval_scope_hash_must_match_grant_resource() {
    let fixture = Fixture::new();
    let approved_file = fixture.write_external_file("approved.md", "# approved");
    let other_file = fixture.write_external_file("other.md", "# other");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let store = CapabilityStore::new();

    let result = store
        .issue_file_read_grant(FileReadGrantInput {
            approval: fixture.approval_for(&approved_file, now, ApprovalStatus::Approved),
            ..fixture.grant_input(&other_file, now)
        })
        .expect("issue grant evaluation");

    assert_eq!(result, Err(CapabilityGrantRejection::ApprovalScopeMismatch));
    assert_eq!(
        store.uses_remaining("cap-source").expect("uses remaining"),
        None
    );
}

#[test]
fn changed_resource_version_rejects_grant_use_without_consuming_it() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("source.md", "# source");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let engine = fixture.engine_at(now);
    engine
        .capability_store()
        .issue_file_read_grant(fixture.grant_input(&external_file, now))
        .expect("issue grant")
        .expect("approved grant");
    fs::write(&external_file, "# replaced source").expect("replace source");

    let evaluation = engine
        .authorize_file_read(&fixture.request_at(&external_file, Some("cap-source"), now))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Deny);
    assert_eq!(
        evaluation.reason,
        PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::ResourceChanged)
    );
    assert_eq!(
        engine
            .capability_store()
            .uses_remaining("cap-source")
            .expect("uses remaining"),
        Some(1)
    );
    assert!(evaluation.audit.grant_fingerprint.is_some());
}

#[test]
fn concurrent_grant_reuse_allows_only_one_consumer() {
    let fixture = Fixture::new();
    let external_file = fixture.write_external_file("source.md", "# source");
    let policy = WorkspacePathPolicy::try_new(fixture.workspace.path()).expect("workspace policy");
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store
        .issue_file_read_grant(fixture.grant_input(&external_file, now))
        .expect("issue grant")
        .expect("approved grant");
    let engine = Arc::new(PolicyEngine::with_capability_store_and_fixed_now(
        policy, store, now,
    ));
    let barrier = Arc::new(Barrier::new(2));

    let handles = (0..2)
        .map(|_| {
            let engine = Arc::clone(&engine);
            let barrier = Arc::clone(&barrier);
            let request = fixture.request_at(&external_file, Some("cap-source"), now);
            thread::spawn(move || {
                barrier.wait();
                engine
                    .authorize_file_read(&request)
                    .expect("policy evaluation")
                    .decision
            })
        })
        .collect::<Vec<_>>();

    let decisions = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread joins"))
        .collect::<Vec<_>>();
    let allowed = decisions
        .iter()
        .filter(|decision| **decision == PolicyDecision::Allow)
        .count();
    let denied = decisions
        .iter()
        .filter(|decision| **decision == PolicyDecision::Deny)
        .count();

    assert_eq!(allowed, 1);
    assert_eq!(denied, 1);
}

struct Fixture {
    workspace: TempDir,
    external: TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            workspace: tempfile::tempdir().expect("workspace tempdir"),
            external: tempfile::tempdir().expect("external tempdir"),
        }
    }

    fn engine(&self) -> PolicyEngine {
        PolicyEngine::new(
            WorkspacePathPolicy::try_new(self.workspace.path()).expect("workspace policy"),
        )
    }

    fn engine_at(&self, now: SystemTime) -> PolicyEngine {
        PolicyEngine::with_fixed_now(
            WorkspacePathPolicy::try_new(self.workspace.path()).expect("workspace policy"),
            now,
        )
    }

    fn write_external_file(&self, name: &str, contents: &str) -> std::path::PathBuf {
        let path = self.external.path().join(name);
        fs::write(&path, contents).expect("write external file");
        path
    }

    fn request(
        &self,
        path: impl Into<std::path::PathBuf>,
        capability_id: Option<&str>,
    ) -> FileReadPolicyRequest {
        self.request_at(
            path,
            capability_id,
            SystemTime::UNIX_EPOCH + Duration::from_secs(100),
        )
    }

    fn request_at(
        &self,
        path: impl Into<std::path::PathBuf>,
        capability_id: Option<&str>,
        _now: SystemTime,
    ) -> FileReadPolicyRequest {
        FileReadPolicyRequest {
            actor_id: "user-1".to_string(),
            workspace_id: "ws-1".to_string(),
            audience: "seaki-source-ingest".to_string(),
            operation: "source.ingest".to_string(),
            path: path.into(),
            capability_id: capability_id.map(str::to_string),
        }
    }

    fn grant_input(&self, path: impl AsRef<Path>, now: SystemTime) -> FileReadGrantInput {
        FileReadGrantInput {
            capability_id: "cap-source".to_string(),
            subject_actor_id: "user-1".to_string(),
            workspace_id: "ws-1".to_string(),
            audience: "seaki-source-ingest".to_string(),
            operation: "source.ingest".to_string(),
            path: path.as_ref().to_path_buf(),
            max_bytes: 1024,
            declared_mime: Some("text/markdown".to_string()),
            not_before: now - Duration::from_secs(1),
            expires_at: now + Duration::from_secs(60),
            granted_by: "local_user".to_string(),
            approval: self.approval_for(path.as_ref(), now, ApprovalStatus::Approved),
        }
    }

    fn approval(&self, now: SystemTime, status: ApprovalStatus) -> ApprovalDecision {
        let path = self.external.path().join("source.md");
        self.approval_for(&path, now, status)
    }

    fn approval_for(
        &self,
        path: impl AsRef<Path>,
        now: SystemTime,
        status: ApprovalStatus,
    ) -> ApprovalDecision {
        let canonical_path = path.as_ref().canonicalize().expect("canonical path");
        let resource = snapshot_file(&canonical_path, 1024)
            .expect("resource snapshot")
            .expect("resource within limit");
        ApprovalDecision {
            approval_id: "approval-source".to_string(),
            policy_decision_id: "policy-source".to_string(),
            scope_hash: file_read_grant_scope_hash(&FileReadGrantScope {
                subject_actor_id: "user-1",
                workspace_id: "ws-1",
                audience: "seaki-source-ingest",
                operation: "source.ingest",
                canonical_path: &canonical_path,
                max_bytes: 1024,
                declared_mime: Some("text/markdown"),
                resource: &resource,
            }),
            decided_by: "local_user".to_string(),
            status,
            decided_at: now,
        }
    }
}

#[test]
fn issue_generic_capability_grant() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let result = store.issue_capability_grant(
        "cap-wiki".to_string(),
        "user-1".to_string(),
        "ws-1".to_string(),
        "pipe.command.wiki.search".to_string(),
        "seaki-pipe".to_string(),
        "wiki.search".to_string(),
        Some(now - Duration::from_secs(1)),
        Some(now + Duration::from_secs(60)),
        1,
        "local_user".to_string(),
    );
    assert!(result.is_ok());
    let handle = result.unwrap().unwrap();
    assert_eq!(handle.capability_id, "cap-wiki");
    assert_eq!(store.generic_uses_remaining("cap-wiki").unwrap(), Some(1));
}

#[test]
fn consume_generic_grant_success() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store
        .issue_capability_grant(
            "cap-wiki".to_string(),
            "user-1".to_string(),
            "ws-1".to_string(),
            "pipe.command.wiki.search".to_string(),
            "seaki-pipe".to_string(),
            "wiki.search".to_string(),
            Some(now - Duration::from_secs(1)),
            Some(now + Duration::from_secs(60)),
            1,
            "local_user".to_string(),
        )
        .unwrap()
        .unwrap();

    let request = GenericUseCapabilityRequest {
        capability_id: "cap-wiki".to_string(),
        subject_actor_id: "user-1".to_string(),
        audience: "seaki-pipe".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        now,
    };

    let result = store.consume_generic_grant(&request).unwrap();
    assert!(result.is_ok());
    assert_eq!(store.generic_uses_remaining("cap-wiki").unwrap(), Some(0));
}

#[test]
fn consume_generic_grant_expired_fails() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store
        .issue_capability_grant(
            "cap-wiki".to_string(),
            "user-1".to_string(),
            "ws-1".to_string(),
            "pipe.command.wiki.search".to_string(),
            "seaki-pipe".to_string(),
            "wiki.search".to_string(),
            Some(now - Duration::from_secs(60)),
            Some(now - Duration::from_secs(1)),
            1,
            "local_user".to_string(),
        )
        .unwrap()
        .unwrap();

    let request = GenericUseCapabilityRequest {
        capability_id: "cap-wiki".to_string(),
        subject_actor_id: "user-1".to_string(),
        audience: "seaki-pipe".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        now,
    };

    let result = store.consume_generic_grant(&request).unwrap();
    assert_eq!(
        result.unwrap_err().rejection,
        CapabilityGrantRejection::Expired
    );
    assert_eq!(store.generic_uses_remaining("cap-wiki").unwrap(), Some(1));
}

#[test]
fn consume_generic_grant_uses_depleted_fails() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    store
        .issue_capability_grant(
            "cap-wiki".to_string(),
            "user-1".to_string(),
            "ws-1".to_string(),
            "pipe.command.wiki.search".to_string(),
            "seaki-pipe".to_string(),
            "wiki.search".to_string(),
            Some(now - Duration::from_secs(1)),
            Some(now + Duration::from_secs(60)),
            0,
            "local_user".to_string(),
        )
        .unwrap()
        .unwrap();

    let request = GenericUseCapabilityRequest {
        capability_id: "cap-wiki".to_string(),
        subject_actor_id: "user-1".to_string(),
        audience: "seaki-pipe".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        now,
    };

    let result = store.consume_generic_grant(&request).unwrap();
    assert_eq!(
        result.unwrap_err().rejection,
        CapabilityGrantRejection::AlreadyUsed
    );
}

#[test]
fn authorize_capability_no_grant_require_approval() {
    let policy = WorkspacePathPolicy::try_new("/tmp").unwrap();
    let engine = PolicyEngine::new(policy);
    let request = CapabilityPolicyRequest {
        actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        capability_id: None,
        side_effect_level: SideEffectLevel::ProposalOnly,
        audience: "seaki-pipe".to_string(),
    };

    let eval = engine.authorize_capability(&request).unwrap();
    assert_eq!(eval.decision, PolicyDecision::RequireApproval);
    assert_eq!(eval.reason, PolicyReason::MissingCapabilityGrant);
}

#[test]
fn authorize_capability_with_grant_allow() {
    let policy = WorkspacePathPolicy::try_new("/tmp").unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let engine = PolicyEngine::with_fixed_now(policy, now);
    engine
        .capability_store()
        .issue_capability_grant(
            "cap-wiki".to_string(),
            "user-1".to_string(),
            "ws-1".to_string(),
            "pipe.command.wiki.search".to_string(),
            "seaki-pipe".to_string(),
            "wiki.search".to_string(),
            Some(now - Duration::from_secs(1)),
            Some(now + Duration::from_secs(60)),
            1,
            "local_user".to_string(),
        )
        .unwrap()
        .unwrap();

    let request = CapabilityPolicyRequest {
        actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        capability_id: Some("cap-wiki".to_string()),
        side_effect_level: SideEffectLevel::SideEffect,
        audience: "seaki-pipe".to_string(),
    };

    let eval = engine.authorize_capability(&request).unwrap();
    assert_eq!(eval.decision, PolicyDecision::Allow);
    assert_eq!(eval.reason, PolicyReason::CapabilityGrant);
    assert_eq!(
        engine
            .capability_store()
            .generic_uses_remaining("cap-wiki")
            .unwrap(),
        Some(0)
    );
}

#[test]
fn authorize_capability_none_level_always_allow() {
    let policy = WorkspacePathPolicy::try_new("/tmp").unwrap();
    let engine = PolicyEngine::new(policy);
    let request = CapabilityPolicyRequest {
        actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        operation: "wiki.search".to_string(),
        capability_id: None,
        side_effect_level: SideEffectLevel::None,
        audience: "seaki-pipe".to_string(),
    };

    let eval = engine.authorize_capability(&request).unwrap();
    assert_eq!(eval.decision, PolicyDecision::Allow);
}

#[test]
fn channel_action_grant_issue_and_consume() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let provenance = Provenance {
        transaction_id: "tx-1".to_string(),
        source_id: "src-1".to_string(),
        citation_ids: vec!["c1".to_string()],
        thread_scope: "thread-1".to_string(),
        audit_id: "audit-1".to_string(),
    };
    let input = IssueChannelActionGrantInput {
        grant_id: "cg-1".to_string(),
        scope: "scope-1".to_string(),
        audience: "aud-1".to_string(),
        ttl: Duration::from_secs(60),
        uses: 2,
        idempotency_key: "idem-1".to_string(),
        allowed_actions: vec!["send".to_string()],
        provenance: provenance.clone(),
        issued_at: now,
    };

    let grant = store
        .issue_channel_action_grant(input)
        .expect("issue ok")
        .expect("grant ok");
    assert_eq!(grant.grant_id, "cg-1");
    assert_eq!(grant.provenance.transaction_id, "tx-1");

    store
        .consume_channel_action_grant("cg-1", now + Duration::from_secs(1))
        .expect("consume ok")
        .expect("first use ok");
    assert_eq!(
        store.channel_action_uses_remaining("cg-1").unwrap(),
        Some(1)
    );

    store
        .consume_channel_action_grant("cg-1", now + Duration::from_secs(2))
        .expect("consume ok")
        .expect("second use ok");
    assert_eq!(
        store.channel_action_uses_remaining("cg-1").unwrap(),
        Some(0)
    );

    let result = store
        .consume_channel_action_grant("cg-1", now + Duration::from_secs(3))
        .expect("consume ok");
    assert_eq!(result, Err(GrantError::UsesExhausted));
}

#[test]
fn channel_action_grant_expired_cannot_consume() {
    let store = CapabilityStore::new();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let input = IssueChannelActionGrantInput {
        grant_id: "cg-1".to_string(),
        scope: "scope-1".to_string(),
        audience: "aud-1".to_string(),
        ttl: Duration::from_secs(10),
        uses: 1,
        idempotency_key: "idem-1".to_string(),
        allowed_actions: vec!["send".to_string()],
        provenance: Provenance {
            transaction_id: "tx-1".to_string(),
            source_id: "src-1".to_string(),
            citation_ids: vec![],
            thread_scope: "thread-1".to_string(),
            audit_id: "audit-1".to_string(),
        },
        issued_at: now,
    };
    store.issue_channel_action_grant(input).unwrap().unwrap();

    let result = store.consume_channel_action_grant("cg-1", now + Duration::from_secs(20));
    assert_eq!(result.unwrap(), Err(GrantError::GrantExpired));
}

#[test]
fn audit_record_grant_issued() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(&tmp, "0123456789").unwrap();
    let resource = snapshot_file(tmp.path(), 1024).unwrap().unwrap();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let grant = CapabilityGrant {
        capability_id: "cap-1".to_string(),
        subject_actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "file.read".to_string(),
        audience: "seaki-source-ingest".to_string(),
        operation: "source.ingest".to_string(),
        canonical_path: tmp.path().to_path_buf(),
        resource,
        max_bytes: 1024,
        declared_mime: Some("text/markdown".to_string()),
        not_before: now,
        expires_at: now + Duration::from_secs(60),
        uses_remaining: 1,
        granted_by: "local_user".to_string(),
        approval_id: "approval-1".to_string(),
        policy_decision_id: "policy-1".to_string(),
        revoked_at: None,
    };
    let record = AuditRecord::grant_issued(&grant);
    assert_eq!(record.action, AuditAction::GrantIssued);
    assert_eq!(record.actor_id, "user-1");
    assert_eq!(record.workspace_id, "ws-1");
    assert_eq!(record.audience, "seaki-source-ingest");
    assert_eq!(record.operation, "source.ingest");
    assert_eq!(record.canonical_path, tmp.path());
    assert_eq!(record.capability_id, Some("cap-1".to_string()));
    assert_eq!(record.decision, PolicyDecision::Allow);
    assert_eq!(record.reason, PolicyReason::CapabilityGrant);
    assert!(record.grant_fingerprint.is_some());
    assert_eq!(record.policy_decision_id, "policy-1");
    assert_eq!(record.occurred_at, now);
}

#[test]
fn audit_record_generic_grant_issued() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let grant = GenericCapabilityGrant {
        capability_id: "cap-wiki".to_string(),
        subject_actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        capability: "pipe.command.wiki.search".to_string(),
        audience: "seaki-pipe".to_string(),
        operation: "wiki.search".to_string(),
        not_before: now,
        expires_at: now + Duration::from_secs(60),
        uses_remaining: 1,
        granted_by: "local_user".to_string(),
        policy_decision_id: "policy-1".to_string(),
        revoked_at: None,
    };
    let record = AuditRecord::generic_grant_issued(&grant);
    assert_eq!(record.action, AuditAction::GrantIssued);
    assert_eq!(record.actor_id, "user-1");
    assert_eq!(record.workspace_id, "ws-1");
    assert_eq!(record.audience, "seaki-pipe");
    assert_eq!(record.operation, "wiki.search");
    assert_eq!(record.canonical_path, PathBuf::new());
    assert_eq!(record.capability_id, Some("cap-wiki".to_string()));
    assert_eq!(record.decision, PolicyDecision::Allow);
    assert_eq!(record.reason, PolicyReason::CapabilityGrant);
    assert!(record.grant_fingerprint.is_some());
    assert_eq!(record.policy_decision_id, "policy-1");
    assert_eq!(record.occurred_at, now);
}

#[test]
fn audit_record_policy_decision() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let request = FileReadPolicyRequest {
        actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        audience: "seaki-source-ingest".to_string(),
        operation: "source.ingest".to_string(),
        path: PathBuf::from("/tmp/test.md"),
        capability_id: None,
    };
    let canonical_path = PathBuf::from("/tmp/test.md");
    let record = AuditRecord::policy_decision(
        &request,
        &canonical_path,
        now,
        PolicyDecision::Deny,
        PolicyReason::PathOutsideWorkspace,
    );
    assert_eq!(record.action, AuditAction::PolicyDecision);
    assert_eq!(record.actor_id, "user-1");
    assert_eq!(record.workspace_id, "ws-1");
    assert_eq!(record.audience, "seaki-source-ingest");
    assert_eq!(record.operation, "source.ingest");
    assert_eq!(record.canonical_path, canonical_path);
    assert_eq!(record.capability_id, None);
    assert_eq!(record.grant_fingerprint, None);
    assert_eq!(record.decision, PolicyDecision::Deny);
    assert_eq!(record.reason, PolicyReason::PathOutsideWorkspace);
    assert_eq!(record.occurred_at, now);
}

#[test]
fn audit_record_capability_consumed() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
    let request = FileReadPolicyRequest {
        actor_id: "user-1".to_string(),
        workspace_id: "ws-1".to_string(),
        audience: "seaki-source-ingest".to_string(),
        operation: "source.ingest".to_string(),
        path: PathBuf::from("/tmp/test.md"),
        capability_id: Some("cap-1".to_string()),
    };
    let canonical_path = PathBuf::from("/tmp/test.md");
    let record = AuditRecord::capability_consumed(
        &request,
        &canonical_path,
        "cap-1",
        Some("fingerprint-1".to_string()),
        now,
        PolicyDecision::Allow,
        PolicyReason::CapabilityGrant,
    );
    assert_eq!(record.action, AuditAction::CapabilityConsumed);
    assert_eq!(record.actor_id, "user-1");
    assert_eq!(record.workspace_id, "ws-1");
    assert_eq!(record.audience, "seaki-source-ingest");
    assert_eq!(record.operation, "source.ingest");
    assert_eq!(record.canonical_path, canonical_path);
    assert_eq!(record.capability_id, Some("cap-1".to_string()));
    assert_eq!(record.grant_fingerprint, Some("fingerprint-1".to_string()));
    assert_eq!(record.decision, PolicyDecision::Allow);
    assert_eq!(record.reason, PolicyReason::CapabilityGrant);
    assert_eq!(record.occurred_at, now);
}

#[test]
fn authorize_file_read_allow() {
    let fixture = Fixture::new();
    let allowed_file = fixture.workspace.path().join("allowed.md");
    fs::write(&allowed_file, "# hello").expect("write allowed file");
    let engine = fixture.engine();

    let evaluation = engine
        .authorize_file_read(&fixture.request(&allowed_file, None))
        .expect("policy evaluation");

    assert_eq!(evaluation.decision, PolicyDecision::Allow);
    assert_eq!(evaluation.reason, PolicyReason::WorkspaceAllowlist);
}

#[test]
fn side_effect_level_from_str_invalid() {
    assert_eq!(
        SideEffectLevel::from_str("invalid"),
        Err("unknown side_effect_level: invalid".to_string())
    );
}

#[test]
fn policy_error_display() {
    let err1 = PolicyError::PathCanonicalizeFailed {
        path: PathBuf::from("/tmp/test"),
        message: "no such file".to_string(),
    };
    assert_eq!(
        err1.to_string(),
        "failed to canonicalize /tmp/test: no such file"
    );

    let err2 = PolicyError::CapabilityStorePoisoned;
    assert_eq!(err2.to_string(), "capability store lock poisoned");

    let err3 = PolicyError::DuplicateCapabilityId("cap-1".to_string());
    assert_eq!(err3.to_string(), "duplicate capability id: cap-1");

    let err4 = PolicyError::UnsupportedCapability("file.write".to_string());
    assert_eq!(err4.to_string(), "unsupported capability: file.write");
}

#[test]
fn grant_error_display() {
    assert_eq!(
        GrantError::DuplicateGrantId("g1".to_string()).to_string(),
        "duplicate grant id: g1"
    );
    assert_eq!(GrantError::GrantNotFound.to_string(), "grant not found");
    assert_eq!(GrantError::GrantExpired.to_string(), "grant expired");
    assert_eq!(GrantError::UsesExhausted.to_string(), "uses exhausted");
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create symlink");
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("create symlink");
}
