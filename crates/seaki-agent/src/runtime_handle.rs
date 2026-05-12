//! Tokio runtime handle for the seaki-agent crate.
//!
//! Provides [`AgentRuntimeHandle`] which wraps a tokio runtime and exposes
//! a [`block_on`](AgentRuntimeHandle::block_on) method for bridging synchronous
//! code with async futures.

use std::sync::Arc;

/// Encapsulates a tokio runtime, providing a `block_on` bridge for sync code.
///
/// Preferentially reuses the current thread's tokio runtime
/// ([`Handle::try_current`]); otherwise creates an owned runtime at
/// construction time. This avoids panics caused by nested `block_on` calls.
///
/// # Panics
///
/// Calling [`block_on`](AgentRuntimeHandle::block_on) from inside a tokio
/// worker thread when the handle was obtained via
/// [`from_current_or_new`](AgentRuntimeHandle::from_current_or_new) will
/// panic, because [`tokio::runtime::Handle::block_on`] cannot be called from
/// an asynchronous context. Ensure `block_on` is only called from
/// non-async code paths.
pub struct AgentRuntimeHandle {
    inner: Arc<AgentRuntimeHandleInner>,
}

enum AgentRuntimeHandleInner {
    /// An owned multi-threaded runtime (created standalone).
    Owned(tokio::runtime::Runtime),
    /// A handle borrowed from an existing tokio runtime.
    Borrowed(tokio::runtime::Handle),
}

impl Default for AgentRuntimeHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRuntimeHandle {
    /// Creates a standalone multi-threaded tokio runtime.
    ///
    /// Use this for tests and standalone processes that do not already have
    /// a tokio runtime.
    ///
    /// # Panics
    ///
    /// Panics if the runtime fails to build (extremely unlikely).
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        Self {
            inner: Arc::new(AgentRuntimeHandleInner::Owned(runtime)),
        }
    }

    /// Attempts to reuse the current tokio runtime; falls back to creating a
    /// new standalone runtime if no runtime is available on the current thread.
    pub fn from_current_or_new() -> Self {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => Self {
                inner: Arc::new(AgentRuntimeHandleInner::Borrowed(handle)),
            },
            Err(_) => Self::new(),
        }
    }

    /// Runs the given future on the runtime, blocking the current thread until
    /// the result is ready.
    ///
    /// # Panics
    ///
    /// Panics if called from within a tokio worker thread when the inner
    /// handle is a [`AgentRuntimeHandleInner::Borrowed`] variant, because
    /// [`tokio::runtime::Handle::block_on`] cannot be called from an
    /// asynchronous context.
    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        match &*self.inner {
            AgentRuntimeHandleInner::Owned(rt) => rt.block_on(f),
            AgentRuntimeHandleInner::Borrowed(handle) => handle.block_on(f),
        }
    }
}

impl Clone for AgentRuntimeHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl std::fmt::Debug for AgentRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.inner {
            AgentRuntimeHandleInner::Owned(_) => f
                .debug_struct("AgentRuntimeHandle")
                .field("type", &"Owned")
                .finish(),
            AgentRuntimeHandleInner::Borrowed(_) => f
                .debug_struct("AgentRuntimeHandle")
                .field("type", &"Borrowed")
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_runtime() {
        let _handle = AgentRuntimeHandle::new();
    }

    #[test]
    fn block_on_executes_future() {
        let handle = AgentRuntimeHandle::new();
        let result = handle.block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn clone_works() {
        let handle = AgentRuntimeHandle::new();
        let cloned = handle.clone();
        let result = cloned.block_on(async { 99 });
        assert_eq!(result, 99);
    }

    #[test]
    fn from_current_or_new_without_runtime() {
        // Outside any tokio runtime, this should create an owned runtime.
        let handle = AgentRuntimeHandle::from_current_or_new();
        let result = handle.block_on(async { "hello" });
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn from_current_or_new_with_runtime() {
        // Inside a tokio runtime, this should borrow the existing handle.
        let handle = AgentRuntimeHandle::from_current_or_new();
        // We cannot call block_on here (would panic inside a tokio worker),
        // but we can verify it was created and matches the borrowed variant.
        assert!(format!("{:?}", handle).contains("Borrowed"));
    }

    #[test]
    fn debug_format_owned() {
        let handle = AgentRuntimeHandle::new();
        let debug_str = format!("{:?}", handle);
        assert!(debug_str.contains("Owned"));
    }

    #[test]
    fn block_on_async_computation() {
        let handle = AgentRuntimeHandle::new();
        let result = handle.block_on(async {
            let a = async { 10 }.await;
            let b = async { 20 }.await;
            a + b
        });
        assert_eq!(result, 30);
    }
}
