use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::types::{PolicyDecision, PolicyError, PolicyReason, PolicyResult};

const DEFAULT_DENY_ROOT_NAMES: &[&str] = &[".git", ".seaki", ".codex", ".agents"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathPolicy {
    workspace_root: PathBuf,
    allow_roots: Vec<PathBuf>,
    deny_roots: Vec<PathBuf>,
}

impl WorkspacePathPolicy {
    /// 创建新的工作区路径策略。
    ///
    /// # Errors
    ///
    /// 当工作区根目录无法 canonicalize 时返回错误。
    pub fn try_new(workspace_root: impl AsRef<Path>) -> PolicyResult<Self> {
        let workspace_root = canonicalize_existing(workspace_root.as_ref())?;
        Ok(Self {
            allow_roots: vec![workspace_root.clone()],
            deny_roots: Vec::new(),
            workspace_root,
        })
    }

    /// 设置额外的允许根目录。
    ///
    /// # Errors
    ///
    /// 当任一允许根目录无法 canonicalize 时返回错误。
    pub fn with_allow_roots(
        mut self,
        allow_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.allow_roots = canonicalize_roots(allow_roots)?;
        Ok(self)
    }

    /// 设置拒绝根目录。
    ///
    /// # Errors
    ///
    /// 当任一拒绝根目录无法 canonicalize 时返回错误。
    pub fn with_deny_roots(
        mut self,
        deny_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.deny_roots = canonicalize_roots(deny_roots)?;
        Ok(self)
    }

    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// 对给定路径进行 canonicalize。
    ///
    /// # Errors
    ///
    /// 当路径无法 canonicalize 时返回错误。
    pub fn canonicalize_path(&self, path: impl AsRef<Path>) -> PolicyResult<PathBuf> {
        canonicalize_existing(path.as_ref())
    }

    #[must_use]
    pub fn is_workspace_read_allowed(&self, canonical_path: &Path) -> bool {
        self.is_allowlisted(canonical_path) && !self.is_denied(canonical_path)
    }

    /// 判断对工作区内给定路径的读取请求应被允许还是拒绝。
    ///
    /// # Errors
    ///
    /// 当路径无法 canonicalize 时返回错误。
    pub fn classify_workspace_read(
        &self,
        path: impl AsRef<Path>,
    ) -> PolicyResult<WorkspacePathDecision> {
        let canonical_path = self.canonicalize_path(path)?;
        let allowed = self.is_allowlisted(&canonical_path);
        let denied = self.is_denied(&canonical_path);
        let decision = if allowed && !denied {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny
        };
        let reason = if denied {
            PolicyReason::PathDenied
        } else if allowed {
            PolicyReason::WorkspaceAllowlist
        } else {
            PolicyReason::PathOutsideWorkspace
        };

        Ok(WorkspacePathDecision {
            canonical_path,
            decision,
            reason,
        })
    }

    fn is_allowlisted(&self, canonical_path: &Path) -> bool {
        self.allow_roots
            .iter()
            .any(|root| path_contains(root, canonical_path))
    }

    fn is_denied(&self, canonical_path: &Path) -> bool {
        self.deny_roots
            .iter()
            .any(|root| path_contains(root, canonical_path))
            || self.is_default_denied(canonical_path)
    }

    fn is_default_denied(&self, canonical_path: &Path) -> bool {
        canonical_path
            .strip_prefix(&self.workspace_root)
            .ok()
            .and_then(|relative_path| relative_path.components().next())
            .and_then(|component| match component {
                std::path::Component::Normal(name) => Some(name),
                _ => None,
            })
            .is_some_and(|name| {
                DEFAULT_DENY_ROOT_NAMES
                    .iter()
                    .any(|denied| name == OsStr::new(denied))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathDecision {
    pub canonical_path: PathBuf,
    pub decision: PolicyDecision,
    pub reason: PolicyReason,
}

pub(crate) fn canonicalize_existing(path: &Path) -> PolicyResult<PathBuf> {
    path.canonicalize()
        .map_err(|error| PolicyError::PathCanonicalizeFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn canonicalize_roots(
    roots: impl IntoIterator<Item = impl AsRef<Path>>,
) -> PolicyResult<Vec<PathBuf>> {
    roots
        .into_iter()
        .map(|root| canonicalize_existing(root.as_ref()))
        .collect()
}

fn path_contains(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}
