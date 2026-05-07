use super::*;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn profile_for_side_effect_maps_correctly() {
    assert_eq!(
        profile_for_side_effect(seaki_policy::SideEffectLevel::None),
        SandboxProfile::ReadOnly
    );
    assert_eq!(
        profile_for_side_effect(seaki_policy::SideEffectLevel::ProposalOnly),
        SandboxProfile::WorkspaceWrite
    );
    assert_eq!(
        profile_for_side_effect(seaki_policy::SideEffectLevel::SideEffect),
        SandboxProfile::WorkspaceWrite
    );
}

#[test]
fn execute_in_sandbox_spawn_mock_command() {
    let plan = SandboxCommandPlan {
        backend: SandboxBackendKind::MacosSeatbelt,
        executable: PathBuf::from("/bin/echo"),
        args: vec!["hello".to_string()],
        profile_source: "(version 1)".to_string(),
    };

    let result = execute_in_sandbox(&plan, b"", 5000).unwrap();
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    assert!(result.stderr.is_empty());
    assert_eq!(result.exit_code, 0);
    assert!(result.audit_records.is_empty());
}

#[test]
fn execute_in_sandbox_timeout_kills() {
    let plan = SandboxCommandPlan {
        backend: SandboxBackendKind::MacosSeatbelt,
        executable: PathBuf::from("/bin/sleep"),
        args: vec!["10".to_string()],
        profile_source: "(version 1)".to_string(),
    };

    let start = std::time::Instant::now();
    let result = execute_in_sandbox(&plan, b"", 100);
    let elapsed = start.elapsed();

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SandboxError::TimeoutExceeded { timeout_ms: 100 }
    ));
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn execute_in_sandbox_exit_code_preserved() {
    let plan = SandboxCommandPlan {
        backend: SandboxBackendKind::MacosSeatbelt,
        executable: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string(), "exit 42".to_string()],
        profile_source: "(version 1)".to_string(),
    };

    let result = execute_in_sandbox(&plan, b"", 5000).unwrap();
    assert_eq!(result.exit_code, 42);
}

#[test]
fn source_ingest_profile_is_networkless() {
    assert_eq!(PRIMARY_M0_BACKEND, "macos-seatbelt");
    assert!(!SandboxProfile::SourceIngest.allows_network());
}

#[test]
fn macos_backend_uses_fixed_sandbox_exec_path_and_denies_network() {
    let temp = tempdir().expect("tempdir");
    let policy = SandboxPolicy::read_only(temp.path()).expect("policy");
    let plan = MacosSeatbeltBackend::command_plan(&policy, &["/bin/cat".to_string()]);

    assert_eq!(plan.backend.as_str(), PRIMARY_M0_BACKEND);
    assert_eq!(plan.executable, PathBuf::from(MACOS_SEATBELT_EXECUTABLE));
    assert!(plan.profile_source.contains("(deny network*)"));
    assert!(plan.profile_source.contains("(deny default)"));
}

#[test]
fn source_ingest_allows_input_read_and_raw_temp_writes() {
    let fixture = SourceIngestFixture::new();
    let policy = SandboxPolicy::source_ingest(&fixture.request()).expect("policy");
    let raw_cas = fixture.raw_cas.path().canonicalize().expect("raw cas");
    let isolated_temp = fixture
        .isolated_temp
        .path()
        .canonicalize()
        .expect("isolated temp");
    let plan = MacosSeatbeltBackend::command_plan(&policy, &["parser".to_string()]);
    assert!(!policy.write_roots.contains(&raw_cas));
    assert!(policy.write_roots.contains(&isolated_temp));
    assert!(!plan.profile_source.contains(&escape_sbpl_path(&raw_cas)));
    assert!(plan
        .profile_source
        .contains(&escape_sbpl_path(&isolated_temp)));

    let run = run_source_ingest(fixture.request(), |ctx| {
        let input = ctx.read_input()?;
        let raw_path = ctx.write_raw_cas("ab/source.blob", &input)?;
        let temp_path = ctx.write_temp("parser/out.txt", b"parsed")?;
        Ok((input, raw_path, temp_path))
    })
    .expect("run");

    let (input, raw_path, temp_path) = run.output.expect("parser output");
    assert_eq!(input, b"source body");
    assert_eq!(fs::read(raw_path).expect("raw"), b"source body");
    assert_eq!(fs::read(temp_path).expect("temp"), b"parsed");
    assert_eq!(run.audit.len(), 3);
    assert!(run
        .audit
        .iter()
        .all(|record| record.decision == SandboxDecision::Allow));
}

#[test]
fn source_ingest_denies_network_and_audits_rejection() {
    let fixture = SourceIngestFixture::new();
    let run = run_source_ingest(fixture.request(), |ctx| {
        let denied = ctx.network_request("https://example.invalid/parser-model");
        assert!(matches!(
            denied,
            Err(SandboxError::Denied {
                operation: SandboxOperation::NetworkRequest,
                reason: SandboxDenyReason::NetworkDenied,
                ..
            })
        ));
        Ok(())
    })
    .expect("run");

    assert!(run.output.is_ok());
    assert_eq!(run.audit.len(), 1);
    assert_eq!(run.audit[0].operation, SandboxOperation::NetworkRequest);
    assert_eq!(run.audit[0].decision, SandboxDecision::Deny);
    assert_eq!(
        run.audit[0].deny_reason,
        Some(SandboxDenyReason::NetworkDenied)
    );
    assert_eq!(
        run.audit[0].network_target.as_deref(),
        Some("https://example.invalid/parser-model")
    );
}

#[test]
fn source_ingest_denies_workspace_write_and_audits_rejection() {
    let fixture = SourceIngestFixture::new();
    let wiki_path = fixture.workspace.path().join("wiki/page.md");
    fs::create_dir_all(wiki_path.parent().expect("parent")).expect("wiki dir");

    let run = run_source_ingest(fixture.request(), |ctx| {
        let denied = ctx.write_path(&wiki_path, b"not allowed");
        assert!(matches!(
            denied,
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::WriteOutsideAllowedRoots,
                ..
            })
        ));
        Ok(())
    })
    .expect("run");

    assert!(!wiki_path.exists());
    assert!(run.output.is_ok());
    assert_eq!(run.audit.len(), 1);
    assert_eq!(run.audit[0].operation, SandboxOperation::FileWrite);
    assert_eq!(run.audit[0].decision, SandboxDecision::Deny);
    assert_eq!(
        run.audit[0].deny_reason,
        Some(SandboxDenyReason::WriteOutsideAllowedRoots)
    );
}

#[test]
fn source_ingest_denies_writing_input_blob() {
    let fixture = SourceIngestFixture::new();
    let input_path = fixture.input_blob.clone();

    let run = run_source_ingest(fixture.request(), |ctx| {
        let denied = ctx.write_path(&input_path, b"mutated");
        assert!(matches!(
            denied,
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::WriteOutsideAllowedRoots
                    | SandboxDenyReason::SourceInputIsReadOnly,
                ..
            })
        ));
        Ok(())
    })
    .expect("run");

    assert_eq!(fs::read(input_path).expect("input"), b"source body");
    assert!(run.output.is_ok());
    assert_eq!(run.audit[0].decision, SandboxDecision::Deny);
}

#[test]
fn source_ingest_raw_cas_is_append_only() {
    let fixture = SourceIngestFixture::new();
    let run = run_source_ingest(fixture.request(), |ctx| {
        ctx.write_raw_cas("ab/source.blob", b"first")?;
        let denied = ctx.write_raw_cas("ab/source.blob", b"second");
        assert!(matches!(denied, Err(SandboxError::Io(_))));
        Ok(())
    })
    .expect("run");

    assert!(run.output.is_ok());
    assert_eq!(
        fs::read(fixture.raw_cas.path().join("ab/source.blob")).expect("raw"),
        b"first"
    );
    assert_eq!(run.audit.len(), 2);
    assert_eq!(run.audit[1].decision, SandboxDecision::Deny);
}

#[test]
fn source_ingest_denies_relative_escape_for_raw_write() {
    let fixture = SourceIngestFixture::new();
    let run = run_source_ingest(fixture.request(), |ctx| {
        let denied = ctx.write_raw_cas("../escape", b"bad");
        assert!(matches!(
            denied,
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::UnsafeRelativePath,
                ..
            })
        ));
        Ok(())
    })
    .expect("run");

    assert!(run.output.is_ok());
    assert_eq!(run.audit.len(), 1);
    assert_eq!(run.audit[0].operation, SandboxOperation::FileWrite);
    assert_eq!(run.audit[0].decision, SandboxDecision::Deny);
    assert_eq!(
        run.audit[0].deny_reason,
        Some(SandboxDenyReason::UnsafeRelativePath)
    );
}

#[test]
fn source_ingest_denies_parent_dir_in_direct_write_target() {
    let fixture = SourceIngestFixture::new();
    let escaped = fixture.isolated_temp.path().join("../escape");
    let run = run_source_ingest(fixture.request(), |ctx| {
        let denied = ctx.write_path(&escaped, b"bad");
        assert!(matches!(
            denied,
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::UnsafeRelativePath,
                ..
            })
        ));
        Ok(())
    })
    .expect("run");

    assert!(run.output.is_ok());
    assert_eq!(run.audit.len(), 1);
    assert_eq!(run.audit[0].decision, SandboxDecision::Deny);
    assert_eq!(
        run.audit[0].deny_reason,
        Some(SandboxDenyReason::UnsafeRelativePath)
    );
}

struct SourceIngestFixture {
    _source_dir: tempfile::TempDir,
    workspace: tempfile::TempDir,
    raw_cas: tempfile::TempDir,
    isolated_temp: tempfile::TempDir,
    input_blob: PathBuf,
}

impl SourceIngestFixture {
    fn new() -> Self {
        let source_dir = tempdir().expect("source tempdir");
        let workspace = tempdir().expect("workspace tempdir");
        let raw_cas = tempdir().expect("raw tempdir");
        let isolated_temp = tempdir().expect("isolated tempdir");
        let input_blob = source_dir.path().join("source.md");
        fs::write(&input_blob, b"source body").expect("write source");

        Self {
            _source_dir: source_dir,
            workspace,
            raw_cas,
            isolated_temp,
            input_blob,
        }
    }

    fn request(&self) -> SourceIngestRequest {
        SourceIngestRequest::new(
            self.input_blob.clone(),
            self.raw_cas.path().to_path_buf(),
            self.isolated_temp.path().to_path_buf(),
        )
        .with_actor("actor-1")
        .with_workspace("workspace-1")
        .with_capability_id("cap-source")
        .with_policy_decision_id("pd-source")
    }
}
