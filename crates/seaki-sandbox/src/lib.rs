use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

pub const PRIMARY_M0_BACKEND: &str = "macos-seatbelt";
pub const MACOS_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxProfile {
    ReadOnly,
    WorkspaceWrite,
    SourceIngest,
}

impl SandboxProfile {
    #[must_use]
    pub const fn allows_network(self) -> bool {
        false
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::SourceIngest => "source-ingest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxBackendKind {
    MacosSeatbelt,
}

impl SandboxBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MacosSeatbelt => PRIMARY_M0_BACKEND,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub profile: SandboxProfile,
    pub read_roots: Vec<PathBuf>,
    pub write_roots: Vec<PathBuf>,
    broker_write_roots: Vec<PathBuf>,
    pub network_allowed: bool,
}

impl SandboxPolicy {
    /// Create a read-only sandbox policy for the given workspace root.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace root cannot be canonicalized or does not exist.
    pub fn read_only(workspace_root: impl AsRef<Path>) -> SandboxResult<Self> {
        Ok(Self {
            profile: SandboxProfile::ReadOnly,
            read_roots: vec![canonicalize_existing(workspace_root.as_ref())?],
            write_roots: Vec::new(),
            broker_write_roots: Vec::new(),
            network_allowed: false,
        })
    }

    /// Create a sandbox policy that allows writes within the workspace root.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace root cannot be canonicalized or does not exist.
    pub fn workspace_write(workspace_root: impl AsRef<Path>) -> SandboxResult<Self> {
        let workspace_root = canonicalize_existing(workspace_root.as_ref())?;
        Ok(Self {
            profile: SandboxProfile::WorkspaceWrite,
            read_roots: vec![workspace_root.clone()],
            write_roots: vec![workspace_root],
            broker_write_roots: Vec::new(),
            network_allowed: false,
        })
    }

    /// Create a sandbox policy for source ingestion based on the request.
    ///
    /// # Errors
    ///
    /// Returns an error if any path in the request cannot be canonicalized or does not exist.
    pub fn source_ingest(request: &SourceIngestRequest) -> SandboxResult<Self> {
        Ok(Self {
            profile: SandboxProfile::SourceIngest,
            read_roots: vec![canonicalize_existing(&request.input_blob)?],
            write_roots: vec![canonicalize_existing(&request.isolated_temp_dir)?],
            broker_write_roots: vec![canonicalize_existing(&request.raw_cas_dir)?],
            network_allowed: false,
        })
    }

    /// Check whether the given path is permitted to be read.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is outside allowed read roots or cannot be canonicalized.
    pub fn permits_read(&self, path: impl AsRef<Path>) -> SandboxResult<PathBuf> {
        let canonical_path = canonicalize_existing(path.as_ref())?;
        if self
            .read_roots
            .iter()
            .any(|root| path_contains(root, &canonical_path))
        {
            Ok(canonical_path)
        } else {
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileRead,
                reason: SandboxDenyReason::ReadOutsideAllowedRoots,
                target: Some(path.as_ref().to_path_buf()),
            })
        }
    }

    /// Check whether the given path is permitted to be written.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is outside allowed write roots or cannot be canonicalized.
    pub fn permits_write(&self, path: impl AsRef<Path>) -> SandboxResult<PathBuf> {
        let canonical_path = canonicalize_write_target(path.as_ref())?;
        if self
            .write_roots
            .iter()
            .any(|root| path_contains(root, &canonical_path))
        {
            Ok(canonical_path)
        } else {
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::WriteOutsideAllowedRoots,
                target: Some(path.as_ref().to_path_buf()),
            })
        }
    }

    fn permits_broker_write(&self, path: impl AsRef<Path>) -> SandboxResult<PathBuf> {
        let canonical_path = canonicalize_write_target(path.as_ref())?;
        if self
            .broker_write_roots
            .iter()
            .any(|root| path_contains(root, &canonical_path))
        {
            Ok(canonical_path)
        } else {
            Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::WriteOutsideAllowedRoots,
                target: Some(path.as_ref().to_path_buf()),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCommandPlan {
    pub backend: SandboxBackendKind,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub profile_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxExecutionResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub audit_records: Vec<SandboxAuditRecord>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MacosSeatbeltBackend;

impl MacosSeatbeltBackend {
    #[must_use]
    pub fn command_plan(policy: &SandboxPolicy, command: &[String]) -> SandboxCommandPlan {
        let profile_source = build_seatbelt_profile(policy);
        let mut args = vec!["-p".to_string(), profile_source.clone()];
        args.extend(command.iter().cloned());

        SandboxCommandPlan {
            backend: SandboxBackendKind::MacosSeatbelt,
            executable: PathBuf::from(MACOS_SEATBELT_EXECUTABLE),
            args,
            profile_source,
        }
    }
}

/// 根据副作用级别选择对应的沙箱 profile。
#[must_use]
pub fn profile_for_side_effect(level: seaki_policy::SideEffectLevel) -> SandboxProfile {
    match level {
        seaki_policy::SideEffectLevel::None => SandboxProfile::ReadOnly,
        seaki_policy::SideEffectLevel::ProposalOnly => SandboxProfile::WorkspaceWrite,
        seaki_policy::SideEffectLevel::SideEffect => SandboxProfile::WorkspaceWrite,
    }
}

/// 在沙箱中执行命令计划。
///
/// # Errors
///
/// 当进程创建失败、超时或 IO 错误时返回错误。
pub fn execute_in_sandbox(
    plan: &SandboxCommandPlan,
    stdin_input: &[u8],
    timeout_ms: u64,
) -> Result<SandboxExecutionResult, SandboxError> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    let child = Arc::new(std::sync::Mutex::new(
        Command::new(&plan.executable)
            .args(&plan.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?,
    ));

    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_clone = Arc::clone(&timed_out);
    let child_clone = Arc::clone(&child);

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(timeout_ms));
        timed_out_clone.store(true, Ordering::Relaxed);
        let _ = child_clone.lock().unwrap().kill();
    });

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    {
        let mut child = child.lock().unwrap();
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_input).map_err(SandboxError::Io)?;
        }
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();
        drop(child);

        if let Some(mut out) = stdout_handle {
            out.read_to_end(&mut stdout).map_err(SandboxError::Io)?;
        }
        if let Some(mut err) = stderr_handle {
            err.read_to_end(&mut stderr).map_err(SandboxError::Io)?;
        }
    }

    let status = {
        let mut child = child.lock().unwrap();
        child.wait().map_err(SandboxError::Io)?
    };

    if timed_out.load(Ordering::Relaxed) && status.code().is_none() {
        return Err(SandboxError::TimeoutExceeded { timeout_ms });
    }

    let exit_code = status.code().unwrap_or(-1);

    Ok(SandboxExecutionResult {
        stdout,
        stderr,
        exit_code,
        audit_records: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIngestRequest {
    pub actor_id: String,
    pub workspace_id: String,
    pub capability_id: Option<String>,
    pub policy_decision_id: Option<String>,
    pub input_blob: PathBuf,
    pub raw_cas_dir: PathBuf,
    pub isolated_temp_dir: PathBuf,
}

impl SourceIngestRequest {
    pub fn new(
        input_blob: impl Into<PathBuf>,
        raw_cas_dir: impl Into<PathBuf>,
        isolated_temp_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            actor_id: String::new(),
            workspace_id: String::new(),
            capability_id: None,
            policy_decision_id: None,
            input_blob: input_blob.into(),
            raw_cas_dir: raw_cas_dir.into(),
            isolated_temp_dir: isolated_temp_dir.into(),
        }
    }

    #[must_use]
    pub fn with_actor(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = actor_id.into();
        self
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace_id: impl Into<String>) -> Self {
        self.workspace_id = workspace_id.into();
        self
    }

    #[must_use]
    pub fn with_capability_id(mut self, capability_id: impl Into<String>) -> Self {
        self.capability_id = Some(capability_id.into());
        self
    }

    #[must_use]
    pub fn with_policy_decision_id(mut self, policy_decision_id: impl Into<String>) -> Self {
        self.policy_decision_id = Some(policy_decision_id.into());
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SourceIngestRun<T> {
    pub output: SandboxResult<T>,
    pub audit: Vec<SandboxAuditRecord>,
}

/// Run a source ingestion with sandboxing.
///
/// # Errors
///
/// Returns an error if sandbox policy creation fails or the parser returns an error.
pub fn run_source_ingest<T>(
    request: SourceIngestRequest,
    parser: impl FnOnce(&mut SourceIngestContext) -> SandboxResult<T>,
) -> SandboxResult<SourceIngestRun<T>> {
    let policy = SandboxPolicy::source_ingest(&request)?;
    let mut context = SourceIngestContext::new(request, policy)?;
    let output = parser(&mut context);

    Ok(SourceIngestRun {
        output,
        audit: context.into_audit(),
    })
}

#[derive(Debug)]
pub struct SourceIngestContext {
    request: SourceIngestRequest,
    policy: SandboxPolicy,
    input_blob: PathBuf,
    raw_cas_dir: PathBuf,
    isolated_temp_dir: PathBuf,
    audit: Vec<SandboxAuditRecord>,
}

impl SourceIngestContext {
    fn new(request: SourceIngestRequest, policy: SandboxPolicy) -> SandboxResult<Self> {
        let input_blob = canonicalize_existing(&request.input_blob)?;
        let raw_cas_dir = canonicalize_existing(&request.raw_cas_dir)?;
        let isolated_temp_dir = canonicalize_existing(&request.isolated_temp_dir)?;

        Ok(Self {
            request,
            policy,
            input_blob,
            raw_cas_dir,
            isolated_temp_dir,
            audit: Vec::new(),
        })
    }

    /// Read the input blob for this ingestion.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the input blob is denied by policy or fails.
    pub fn read_input(&mut self) -> SandboxResult<Vec<u8>> {
        let input_blob = self.input_blob.clone();
        self.read_path(input_blob)
    }

    /// Read data from the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is outside allowed read roots or reading fails.
    pub fn read_path(&mut self, path: impl AsRef<Path>) -> SandboxResult<Vec<u8>> {
        let target = path.as_ref();
        let result = self.policy.permits_read(target).and_then(|canonical_path| {
            if self.policy.profile == SandboxProfile::SourceIngest
                && canonical_path != self.input_blob
            {
                return Err(SandboxError::Denied {
                    operation: SandboxOperation::FileRead,
                    reason: SandboxDenyReason::ReadOutsideSourceInput,
                    target: Some(target.to_path_buf()),
                });
            }
            fs::read(&canonical_path).map_err(SandboxError::Io)
        });

        self.record_from_result(SandboxOperation::FileRead, Some(target), &result);
        result
    }

    /// Write bytes to the raw CAS directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the relative path is unsafe, writing is denied by policy, or the write fails.
    pub fn write_raw_cas(
        &mut self,
        relative_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> SandboxResult<PathBuf> {
        let target = match Self::join_sandbox_relative(&self.raw_cas_dir, relative_path.as_ref()) {
            Ok(target) => target,
            Err(error) => {
                let result = Err(error);
                self.record_from_result(
                    SandboxOperation::FileWrite,
                    Some(relative_path.as_ref()),
                    &result,
                );
                return result;
            }
        };
        self.write_path_inner(&target, bytes, WriteMode::AppendOnly)
    }

    /// Write bytes to the isolated temp directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the relative path is unsafe, writing is denied by policy, or the write fails.
    pub fn write_temp(
        &mut self,
        relative_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> SandboxResult<PathBuf> {
        let target =
            match Self::join_sandbox_relative(&self.isolated_temp_dir, relative_path.as_ref()) {
                Ok(target) => target,
                Err(error) => {
                    let result = Err(error);
                    self.record_from_result(
                        SandboxOperation::FileWrite,
                        Some(relative_path.as_ref()),
                        &result,
                    );
                    return result;
                }
            };
        self.write_path_inner(&target, bytes, WriteMode::Replace)
    }

    #[cfg(test)]
    fn write_path(&mut self, path: impl AsRef<Path>, bytes: &[u8]) -> SandboxResult<PathBuf> {
        self.write_path_inner(path.as_ref(), bytes, WriteMode::Replace)
    }

    /// Request network access for the given target.
    ///
    /// # Errors
    ///
    /// Returns an error if network access is not allowed by the sandbox policy.
    pub fn network_request(&mut self, target: impl Into<String>) -> SandboxResult<()> {
        let target = target.into();
        let result = if self.policy.network_allowed {
            Ok(())
        } else {
            Err(SandboxError::Denied {
                operation: SandboxOperation::NetworkRequest,
                reason: SandboxDenyReason::NetworkDenied,
                target: None,
            })
        };
        self.audit.push(SandboxAuditRecord::from_result(
            &self.request,
            self.policy.profile,
            SandboxOperation::NetworkRequest,
            None,
            Some(target),
            &result,
        ));
        result
    }

    #[must_use]
    pub fn audit(&self) -> &[SandboxAuditRecord] {
        &self.audit
    }

    fn into_audit(self) -> Vec<SandboxAuditRecord> {
        self.audit
    }

    fn write_path_inner(
        &mut self,
        path: &Path,
        bytes: &[u8],
        write_mode: WriteMode,
    ) -> SandboxResult<PathBuf> {
        let permitted_write = if self.policy.profile == SandboxProfile::SourceIngest
            && matches!(write_mode, WriteMode::AppendOnly)
        {
            self.policy.permits_broker_write(path)
        } else {
            self.policy.permits_write(path)
        };

        let result = permitted_write.and_then(|canonical_path| {
            if self.policy.profile == SandboxProfile::SourceIngest
                && canonical_path == self.input_blob
            {
                return Err(SandboxError::Denied {
                    operation: SandboxOperation::FileWrite,
                    reason: SandboxDenyReason::SourceInputIsReadOnly,
                    target: Some(path.to_path_buf()),
                });
            }

            if let Some(parent) = canonical_path.parent() {
                fs::create_dir_all(parent).map_err(SandboxError::Io)?;
            }

            match write_mode {
                WriteMode::AppendOnly => {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&canonical_path)
                        .map_err(SandboxError::Io)?;
                    io::Write::write_all(&mut file, bytes).map_err(SandboxError::Io)?;
                }
                WriteMode::Replace => {
                    fs::write(&canonical_path, bytes).map_err(SandboxError::Io)?;
                }
            }

            Ok(canonical_path)
        });

        self.record_from_result(SandboxOperation::FileWrite, Some(path), &result);
        result
    }

    fn join_sandbox_relative(root: &Path, relative_path: &Path) -> SandboxResult<PathBuf> {
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(SandboxError::Denied {
                operation: SandboxOperation::FileWrite,
                reason: SandboxDenyReason::UnsafeRelativePath,
                target: Some(relative_path.to_path_buf()),
            });
        }

        Ok(root.join(relative_path))
    }

    fn record_from_result<T>(
        &mut self,
        operation: SandboxOperation,
        target: Option<&Path>,
        result: &SandboxResult<T>,
    ) {
        self.audit.push(SandboxAuditRecord::from_result(
            &self.request,
            self.policy.profile,
            operation,
            target.map(Path::to_path_buf),
            None,
            result,
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    AppendOnly,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxOperation {
    FileRead,
    FileWrite,
    NetworkRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxDenyReason {
    ReadOutsideAllowedRoots,
    ReadOutsideSourceInput,
    WriteOutsideAllowedRoots,
    SourceInputIsReadOnly,
    UnsafeRelativePath,
    NetworkDenied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxAuditRecord {
    pub profile: SandboxProfile,
    pub operation: SandboxOperation,
    pub decision: SandboxDecision,
    pub deny_reason: Option<SandboxDenyReason>,
    pub actor_id: String,
    pub workspace_id: String,
    pub capability_id: Option<String>,
    pub policy_decision_id: Option<String>,
    pub path: Option<PathBuf>,
    pub network_target: Option<String>,
    pub recorded_at: SystemTime,
}

impl SandboxAuditRecord {
    fn from_result<T>(
        request: &SourceIngestRequest,
        profile: SandboxProfile,
        operation: SandboxOperation,
        path: Option<PathBuf>,
        network_target: Option<String>,
        result: &SandboxResult<T>,
    ) -> Self {
        let (decision, deny_reason) = match result {
            Ok(_) => (SandboxDecision::Allow, None),
            Err(SandboxError::Denied { reason, .. }) => (SandboxDecision::Deny, Some(*reason)),
            Err(
                SandboxError::Io(_)
                | SandboxError::PathCanonicalizeFailed { .. }
                | SandboxError::SpawnFailed(_)
                | SandboxError::TimeoutExceeded { .. },
            ) => (SandboxDecision::Deny, None),
        };

        Self {
            profile,
            operation,
            decision,
            deny_reason,
            actor_id: request.actor_id.clone(),
            workspace_id: request.workspace_id.clone(),
            capability_id: request.capability_id.clone(),
            policy_decision_id: request.policy_decision_id.clone(),
            path,
            network_target,
            recorded_at: SystemTime::now(),
        }
    }
}

#[derive(Debug)]
pub enum SandboxError {
    Denied {
        operation: SandboxOperation,
        reason: SandboxDenyReason,
        target: Option<PathBuf>,
    },
    Io(io::Error),
    PathCanonicalizeFailed {
        path: PathBuf,
        message: String,
    },
    SpawnFailed(String),
    TimeoutExceeded {
        timeout_ms: u64,
    },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied {
                operation,
                reason,
                target,
            } => {
                if let Some(target) = target {
                    write!(
                        f,
                        "{operation:?} denied for {}: {reason:?}",
                        target.display()
                    )
                } else {
                    write!(f, "{operation:?} denied: {reason:?}")
                }
            }
            Self::Io(error) => write!(f, "{error}"),
            Self::PathCanonicalizeFailed { path, message } => {
                write!(f, "failed to canonicalize {}: {message}", path.display())
            }
            Self::SpawnFailed(message) => write!(f, "failed to spawn process: {message}"),
            Self::TimeoutExceeded { timeout_ms } => {
                write!(f, "process timed out after {timeout_ms} ms")
            }
        }
    }
}

impl std::error::Error for SandboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Denied { .. }
            | Self::PathCanonicalizeFailed { .. }
            | Self::SpawnFailed(_)
            | Self::TimeoutExceeded { .. } => None,
        }
    }
}

impl PartialEq for SandboxError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Denied {
                    operation,
                    reason,
                    target,
                },
                Self::Denied {
                    operation: other_operation,
                    reason: other_reason,
                    target: other_target,
                },
            ) => operation == other_operation && reason == other_reason && target == other_target,
            (
                Self::PathCanonicalizeFailed { path, message },
                Self::PathCanonicalizeFailed {
                    path: other_path,
                    message: other_message,
                },
            ) => path == other_path && message == other_message,
            (Self::Io(error), Self::Io(other_error)) => {
                error.kind() == other_error.kind() && error.to_string() == other_error.to_string()
            }
            (Self::SpawnFailed(a), Self::SpawnFailed(b)) => a == b,
            (Self::TimeoutExceeded { timeout_ms: a }, Self::TimeoutExceeded { timeout_ms: b }) => {
                a == b
            }
            _ => false,
        }
    }
}

impl Eq for SandboxError {}

impl From<io::Error> for SandboxError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub type SandboxResult<T> = Result<T, SandboxError>;

fn canonicalize_existing(path: &Path) -> SandboxResult<PathBuf> {
    path.canonicalize()
        .map_err(|error| SandboxError::PathCanonicalizeFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn canonicalize_write_target(path: &Path) -> SandboxResult<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(SandboxError::Denied {
            operation: SandboxOperation::FileWrite,
            reason: SandboxDenyReason::UnsafeRelativePath,
            target: Some(path.to_path_buf()),
        });
    }

    if path.exists() {
        return canonicalize_existing(path);
    }

    let mut missing_components: Vec<OsString> = Vec::new();
    let mut existing_ancestor = path;
    while !existing_ancestor.exists() {
        let Some(file_name) = existing_ancestor.file_name() else {
            return Err(SandboxError::PathCanonicalizeFailed {
                path: path.to_path_buf(),
                message: "write target must be under an existing directory".to_string(),
            });
        };
        missing_components.push(file_name.to_os_string());
        existing_ancestor = existing_ancestor.parent().unwrap_or_else(|| Path::new("."));
    }

    let mut canonical_path = canonicalize_existing(existing_ancestor)?;
    for component in missing_components.iter().rev() {
        canonical_path.push(component);
    }
    Ok(canonical_path)
}

fn path_contains(root: &Path, candidate: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

fn build_seatbelt_profile(policy: &SandboxPolicy) -> String {
    let mut lines = vec![
        "(version 1)".to_string(),
        "(deny default)".to_string(),
        "(allow process*)".to_string(),
        "(allow file-read-metadata)".to_string(),
        "(deny network*)".to_string(),
    ];

    for root in &policy.read_roots {
        lines.push(format!("(allow file-read* {})", sbpl_path_filter(root)));
    }

    for root in &policy.write_roots {
        lines.push(format!("(allow file-write* {})", sbpl_path_filter(root)));
    }

    lines.join("\n")
}

fn escape_sbpl_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn sbpl_path_filter(path: &Path) -> String {
    let filter = if path.is_file() { "literal" } else { "subpath" };
    format!("({filter} \"{}\")", escape_sbpl_path(path))
}

#[cfg(test)]
mod tests;
