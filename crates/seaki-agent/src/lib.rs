pub mod dispatch;
pub mod llm;
pub mod mcp;
pub mod runtime;
pub mod runtime_handle;
pub mod session;
pub mod skill;
pub mod wal;

pub use dispatch::*;
pub use llm::*;
pub use runtime::*;
pub use session::*;
pub use skill::*;
pub use wal::*;

/// 安全截断字符串，按字符数截取，避免 UTF-8 字节边界 panic。
#[must_use]
pub fn safe_truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests;
