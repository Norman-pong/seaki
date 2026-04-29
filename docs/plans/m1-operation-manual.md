# M1 端到端验收与发布门禁操作手册

[返回任务计划](m1-task-plan.md)

本手册记录 M1 端到端验收步骤、质量门禁命令和已知限制清单。M1 交付范围覆盖 Pipeline dry-run、Session Search / Project Note、Fake Channel 入站与 Outbox、以及低信任 Data Block 注入边界验证四条 happy path。

## 环境准备

```bash
# Rust (stable toolchain)
rustup component add rustfmt clippy

# Node.js 22.12+ 与 pnpm
node --version  # >= 22.12.0
pnpm --version  # >= 9.0.0

# 安装前端依赖
pnpm install --frozen-lockfile
```

## 质量门禁（每次提交前执行）

```bash
# Rust
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

# TypeScript / Electron
pnpm check    # 聚合 dto:check + format:check + lint + typecheck + build + test
```

## 运行 M1 相关测试

```bash
# 全部 workspace 测试（含 M1 E2E）
cargo test --workspace

# 单独运行 M1 E2E smoke tests
cargo test m1_pipe_dry_run_produces_proposal_artifact
cargo test m1_memory_note_lifecycle_with_source_checking
cargo test m1_session_search_indexes_redacted_manifest
cargo test m1_memory_propose_does_not_hot_replace_session_prompt
cargo test m1_channel_bridge_webhook_to_outbox_happy_path
```

## Happy Path 演示

### 1. Pipeline dry-run + Proposal Artifact

```rust
// 代码示例（对应测试 m1_pipe_dry_run_produces_proposal_artifact）
let ledger = CoreLedger::open_in_memory().unwrap();

// 验证 builtin 命令列表
let commands = ledger.pipe_list(None);
assert!(commands.iter().any(|c| c.command_id == "wiki.search"));

// 验证 manifest 完整
let manifest = ledger.pipe_inspect("wiki.search").unwrap();
assert!(manifest.validate_schema_hash());

// 构造 proposal pipeline
let ast = seaki_pipe::PipelineAst {
    pipeline_id: "demo-pipe".to_string(),
    steps: vec![
        PipelineStep { command_id: "wiki.search".to_string(), ... },
        PipelineStep { command_id: "citation.resolve".to_string(), ... },
        PipelineStep { command_id: "wiki.patch.propose".to_string(), ... },
    ],
};

// 执行 dry-run
let result = ledger.pipe_dry_run(ast, json!({"keyword": "rust"})).unwrap();
assert!(result.proposal_artifact.is_some());
assert_eq!(ledger.event_count().unwrap(), initial_events); // 无副作用
```

手动验证命令：

```bash
cargo test m1_pipe_dry_run_produces_proposal_artifact -- --nocapture
```

### 2. Session Search + Project Note

#### 2a Project Note 生命周期与 source checking

```rust
// 代码示例（对应测试 m1_memory_note_lifecycle_with_source_checking）
let mut ledger = CoreLedger::open_in_memory().unwrap();

// 创建 note（产生 memory.proposed 事件）
ledger.append_memory_propose(...).unwrap();
assert_eq!(ledger.memory_note("note-1").unwrap().unwrap().status, "proposed");

// BM25 搜索（跨 seaki-memory + seaki-index）
let mut store = NoteStore::new();
let mut index = Bm25CandidateIndex::new();
store.create_note(...);
store.rebuild_index(&mut index, &scope).unwrap();
let results = store.search_notes("ownership", &scope, &index, 10);
assert!(!results.is_empty());

// source checking 冲突 -> 降级为 Conflict
let note = ledger.memory_source_check("note-1", &["borrowing"]).unwrap();
assert_eq!(note.status, "conflict");
```

手动验证命令：

```bash
cargo test m1_memory_note_lifecycle_with_source_checking -- --nocapture
```

#### 2b Redacted Session Manifest 索引与 TTL

```rust
// 代码示例（对应测试 m1_session_search_indexes_redacted_manifest）
let manifest = RedactedSessionManifest::new(
    "session-1",
    "user asked about rust ownership",
    scope.clone(),
    "ref://original-transcript-1",
);
sessions.index_redacted_session(manifest, &mut index).unwrap();

// 搜索返回 candidate
let results = sessions.search_sessions("rust", &scope, &index, 10).unwrap();
assert_eq!(results[0].session_id, "session-1");

// TTL 过期 -> 标记 expired -> grace period 后物理删除
let actions = sessions.cleanup_expired_sessions(now, &mut index).unwrap();
```

手动验证命令：

```bash
cargo test m1_session_search_indexes_redacted_manifest -- --nocapture
```

### 3. Fake Channel 入站 + Webhook + Outbox

```rust
// 代码示例（对应测试 m1_channel_bridge_webhook_to_outbox_happy_path）
let provider = FakeChannelProvider::new();
provider.upsert_binding(binding);

// 合法 webhook 通过
let event = provider.submit_event(payload, &sig, now, "evt-1", ...).unwrap();

// 同一 event_id 再次提交 -> EventReplayed
assert_eq!(provider.submit_event(...), Err(WebhookError::EventReplayed));

// guest role 申请 grant 被 policy 拒绝
let grant_store = ChannelResourceGrantStore::new();
assert_eq!(
    grant_store.issue("guest", grant),
    Err(GrantError::PolicyDeniedInsufficientRole)
);

// Outbox 幂等性
outbox.enqueue(item).unwrap();
outbox.transition(..., OutboxStatus::Sent).unwrap();
assert_eq!(outbox.enqueue(duplicate), Err("idempotency key already sent"));

// Unknown -> query -> Retry
outbox.resolve_unknown("o3", &NotFoundQueryAPI).unwrap();

// 并发 lease 仅一人成功
let wins = concurrent_lease_workers.iter().filter(|&&b| b).count();
assert_eq!(wins, 1);
```

手动验证命令：

```bash
cargo test m1_channel_bridge_webhook_to_outbox_happy_path -- --nocapture
```

### 4. 低信任 Data Block 注入边界验证

```rust
// 代码示例（对应测试 m1_memory_propose_does_not_hot_replace_session_prompt）
ledger.append_memory_propose(...).unwrap();

let events = ledger.replay_events_after(0).unwrap();
assert!(events.iter().any(|e| e.event_type == MEMORY_PROPOSE_EVENT_TYPE));
assert!(!events.iter().any(|e| e.event_type == "prompt.replace"));
```

手动验证命令：

```bash
cargo test m1_memory_propose_does_not_hot_replace_session_prompt -- --nocapture
```

## 已知限制和 M2 前置依赖

| 限制/依赖 | 说明 | M2 计划 |
|---|---|---|
| Pipeline 实际执行（非 dry-run） | M1 仅验证 compose + dry-run 事件流；真实 side-effect 执行器未实现 | M2 实现 `pipe_execute` 与资源配额监控 |
| Note 自动索引到 CoreLedger | M1 中 NoteStore 与 CoreLedger 的 BM25 索引需手动同步 | M2 在 `append_memory_propose/commit` 后自动触发索引更新 |
| Channel 真实网络 I/O | FakeChannelProvider 为内存模拟；无真实 HTTP/webhook 服务端 | M2 接入真实 channel provider（如 Slack/Discord API） |
| Outbox 持久化 | Outbox 为内存结构；daemon 重启后状态丢失 | M2 将 Outbox 持久化到 SQLite 并支持 crash recovery |
| Session TTL 物理删除审计 | `cleanup_expired_sessions` 返回 `PhysicallyDelete` 动作，但 CoreLedger 尚未消费该动作生成 AuditEvent | M2 在 daemon 后台任务中闭环执行 |
| Prompt hot-replace 策略 | M1 仅验证边界（不产生 `prompt.replace`）；实际 prompt 版本管理未实现 | M2 设计 prompt versioning 与影子替换策略 |

## 交付物检查表

- [x] Pipeline dry-run 跨 crate E2E smoke test（`seaki-core`）
- [x] Session Search + Project Note 跨 crate E2E smoke test（`seaki-core` + `seaki-memory` + `seaki-index`）
- [x] Fake Channel 入站/Webhook/Outbox 跨 crate E2E smoke test（`seaki-daemon/tests/`）
- [x] 低信任 Data Block 注入边界验证（`seaki-core`）
- [x] M1 操作手册与已知限制清单
- [x] `cargo test --workspace` 全绿
