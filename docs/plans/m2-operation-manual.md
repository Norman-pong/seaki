# M2 端到端验收与发布门禁操作手册

[返回任务计划](m2-task-plan.md)

本手册记录 M2 端到端验收步骤、质量门禁命令和已知限制清单。M2 交付范围覆盖 Pipeline Designer 编译器与真实执行运行时、Agent Runtime 与 MCP 适配、Channel Bridge（飞书插件）、自动 Memory 与 Review Learning，以及前端配套 UI。

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
cargo clippy --workspace --tests -- -D warnings
cargo test --workspace

# TypeScript / Electron
pnpm typecheck
pnpm lint
pnpm test

# DTO 生成物一致性（如修改了 Rust DTO）
pnpm dto:check
```

## M2 关键 E2E 测试运行

### 全部 workspace 测试

```bash
cargo test --workspace
```

当前全 workspace 共 44 个测试套件、551+ 个测试用例全部通过。

### Pipeline Designer 与执行运行时

```bash
# 意图编译 + 类型检查 + 权限预估 + 成本估算
cargo test intent:: -- --nocapture
cargo test compiler:: -- --nocapture
cargo test cost:: -- --nocapture
cargo test policy:: -- --nocapture

# DAG 控制流与容错
cargo test dag:: -- --nocapture
cargo test checkpoint:: -- --nocapture
cargo test compensate:: -- --nocapture

# Approval Gate 与状态机
cargo test approval_gate:: -- --nocapture
cargo test state_machine:: -- --nocapture

# 真实执行（非 dry-run）
cargo test run:: -- --nocapture
cargo test executor:: -- --nocapture
```

### Agent Runtime

```bash
# Agent 完整闭环：intent → pipeline → execute → answer
cargo test --test agent_runtime -- --nocapture

# Session 状态机与 Compaction
cargo test session_state:: -- --nocapture
cargo test compaction:: -- --nocapture

# Skills 调度
cargo test dispatch:: -- --nocapture
cargo test skill:: -- --nocapture

# LLM 调用
cargo test llm:: -- --nocapture
```

### MCP 适配

```bash
# mcp-to-pipe
cargo test --test mcp_adapter -- --nocapture

# pipe-to-mcp
cargo test --test pipe_to_mcp -- --nocapture

# MCP Client / Protocol / Transport
cargo test --test mcp_client -- --nocapture
cargo test --test mcp_protocol -- --nocapture
cargo test --test mcp_transport -- --nocapture
```

### Channel Bridge

```bash
# WASM 插件运行时 + Secret Broker
cargo test --test plugin_runtime -- --nocapture
cargo test --test plugin_manifest -- --nocapture
cargo test --test plugin_registry -- --nocapture
cargo test --test secret_broker -- --nocapture

# Ingress 归一化 + 身份映射
cargo test --test ingress -- --nocapture

# Quarantine 与资源授权
cargo test --test quarantine -- --nocapture

# Outbox 调度器
cargo test --test outbox_dispatcher -- --nocapture

# 飞书适配器
cargo test --test feishu_adapter -- --nocapture

# Channel 集成 E2E
cargo test --test channel_integration -- --nocapture
cargo test --test m1_channel_e2e -- --nocapture
```

### Memory 系统

```bash
# 自动收集 + Conflict Detection
cargo test memory_collector:: -- --nocapture
cargo test conflict_detector:: -- --nocapture

# Frozen Snapshot + 写入管道
cargo test frozen_snapshot:: -- --nocapture
cargo test propose_pipeline:: -- --nocapture

# 遗忘曲线 + 复习调度 + Grading
cargo test retention:: -- --nocapture
cargo test grading:: -- --nocapture
cargo test review_queue:: -- --nocapture

# 卡片生成 + Topic Clustering + Runbook
cargo test card_generator:: -- --nocapture
cargo test topic_clustering:: -- --nocapture
cargo test runbook_index:: -- --nocapture
```

### 前端组件测试

```bash
pnpm test
```

当前共 8 个测试文件、62 个测试用例全部通过：

| 测试文件 | 用例数 | 覆盖范围 |
|---------|--------|----------|
| `PipelinePanel.test.ts` | 7 | Pipeline 步骤结构、dry-run 预览、事件流、状态颜色 |
| `ChatPanel.test.tsx` | 7 | Skill 选择、消息发送、Pipeline 联动、Approval 交互 |
| `MemoryReviewPanel.test.tsx` | 4 | 到期卡片、显示答案、Grading、空状态 |
| `ChannelPanel.test.tsx` | 4 | Channel 列表、连接状态、事件日志、开关切换 |
| `appModel.test.ts` | 11 | Approval diff 状态机、Domain Client 交互 |
| `packages/transport` | 4 | Transport 层 |
| `packages/state` | 18 | State 管理 |
| `packages/domain` | 7 | Domain 逻辑 |

## Happy Path 演示

### 1. Agent Intent → Pipeline → Execution → Answer

```rust
// crates/seaki-agent/tests/agent_runtime.rs
// execute_intent_full_execution_success
let runtime = AgentRuntime::builder()
    .llm_client(Box::new(MockLlmClient::new("propose pipeline")))
    .skill_registry(registry)
    .build();

let result = runtime.execute_intent(intent, &mut session).await.unwrap();
assert!(result.answer.contains("answer"));
assert_eq!(session.state(), SessionState::Idle);
```

手动验证：
```bash
cargo test --test agent_runtime execute_intent_full_execution_success -- --nocapture
```

### 2. Pipeline Designer 编译器

```rust
// crates/seaki-pipeline/tests/
let parser = MockIntentParser::new();
let graph = parser.parse("search rust ownership and propose patch").unwrap();

let compiler = PipelineCompiler::new(&registry);
let compiled = compiler.compile(&graph).unwrap(); // 类型检查通过

let policy = PolicyEstimator::estimate(&compiled, &actor_caps);
assert_eq!(policy.decision, PolicyDecision::Allow); // 权限足够
```

手动验证：
```bash
cargo test compiler_accepts_valid_linear_pipeline -- --nocapture
cargo test compile_dag_with_tee_branch_join -- --nocapture
cargo test cost_estimate_search_summarize -- --nocapture
```

### 3. 飞书 Channel 入站 → Quarantine → Outbox

```rust
// crates/seaki-channel/tests/feishu_adapter.rs
// feishu_event_to_channel_event_mapping
let event = parse_feishu_event(payload).unwrap();
let channel_event = feishu_event_to_channel_event(event).unwrap();
assert_eq!(channel_event.event_type, "message.received");

// Outbox 调度
let item = OutboxItem::new(...);
outbox.enqueue(item).unwrap();
dispatcher.run_once().unwrap();
assert_eq!(outbox.get(item.id).unwrap().status, OutboxStatus::Sent);
```

手动验证：
```bash
cargo test --test feishu_adapter feishu_event_to_channel_event_mapping -- --nocapture
cargo test --test outbox_dispatcher dispatcher_leases_and_sends_pending -- --nocapture
```

### 4. Memory 自动收集 → Review → Grading

```rust
// crates/seaki-memory/tests/
let collector = MemoryCollector::new();
let items = collector.extract_from_session(&session);
assert!(!items.is_empty());

// Review card 生成与调度
let card = ReviewCard::from_memory_item(&item);
let queue = ReviewQueue::new();
queue.enqueue(card);
let due = queue.due_cards(now);
assert!(!due.is_empty());

// Grading
let engine = GradingEngine::new();
let result = engine.grade(&card, Grade::Good, now);
assert!(result.new_stability_days > card.stability_days);
```

手动验证：
```bash
cargo test collector_extracts_preferences_from_session -- --nocapture
cargo test review_queue_enqueue_and_due -- --nocapture
cargo test grading_easy_increases_stability -- --nocapture
```

### 5. 前端 Happy Path

```bash
# Pipeline UI
pnpm test -- --run PipelinePanel

# Agent Chat + Skill
pnpm test -- --run ChatPanel

# Memory Review
pnpm test -- --run MemoryReviewPanel

# Channel 管理
pnpm test -- --run ChannelPanel
```

## Reject Path 回归测试

### Pipeline 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 类型不匹配 | `cargo test compose_rejects_type_mismatch -- --nocapture` | 编译期拒绝 |
| Cardinality 冲突 | `cargo test compose_rejects_cardinality_conflict -- --nocapture` | 编译期拒绝 |
| 未知命令 | `cargo test compose_rejects_unknown_command -- --nocapture` | 编译期拒绝 |
| 循环依赖 | `cargo test compose_rejects_cycle -- --nocapture` | 编译期拒绝 |
| 资源超限 | `cargo test run_resource_exceeded_terminates -- --nocapture` | `ResourceExceeded` 终止 |
| Approval 拒绝 | `cargo test execute_intent_approval_denied -- --nocapture` | 触发 compensating action |

### Agent 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| Skill 未匹配 | `cargo test execute_intent_no_matching_skill -- --nocapture` | 返回错误 |
| Pipeline 编译失败 | `cargo test execute_intent_pipeline_compile_failed -- --nocapture` | 返回错误 |
| 缺失 Capability | `cargo test dispatch_missing_capability_rejected -- --nocapture` | Skill 被拒绝 |
| 无效状态转换 | `cargo test session_invalid_transition_rejected -- --nocapture` | 状态机拒绝 |

### Channel 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 伪造签名 | `cargo test normalize_rejects_forged_signature -- --nocapture` | Ingress 拒绝 |
| 过期时间戳 | `cargo test normalize_rejects_expired_timestamp -- --nocapture` | Ingress 拒绝 |
| 重复 Event ID | `cargo test concurrent_replay_allows_only_one_success -- --nocapture` | `EventReplayed` |
| Guest 权限不足 | `cargo test guest_is_denied_resource_grant -- --nocapture` | Grant 被拒绝 |
| Quarantine 失败 | `cargo test pipeline_mime_mismatch_detected -- --nocapture` | 进入审计 |

### Memory 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| Source 冲突 | `cargo test source_conflict_downgrades_note_to_conflict -- --nocapture` | 降级为 Conflict |
| Injection 攻击 | `cargo test pipeline_injection_scan_detects_attack -- --nocapture` | Proposal 被拒绝 |
| 过长内容 | `cargo test pipeline_policy_check_rejects_long_content -- --nocapture` | Policy 拒绝 |
| Again 多次后 Relink | `cargo test grading_relink_after_repeated_failure -- --nocapture` | 推荐 `RelinkToSource` |

## 已知限制和 M3 前置依赖

| 限制/依赖 | 说明 | M3 计划 |
|---|---|---|
| LLM 真实调用 | Agent Runtime 中 `OpenAiClient` 为 stub；实际 LLM 调用需配置 API key 和网络 | M3 接入真实 LLM provider（OpenAI / Claude / 本地模型） |
| Electron 与 Rust 桥接 | 前端使用 mock transport；真实场景需通过 Tauri IPC 或 Electron 原生消息与 Rust daemon 通信 | M3 实现前端 ↔ daemon 真实桥接 |
| Channel 多插件 | 仅实现飞书插件；Slack / 企业微信 / Discord 未接入 | M3 按需扩展 IM 插件 |
| Pipeline 分布式执行 | 当前为单进程执行；无跨机器调度 | M3+ 暂不做 |
| Agent 自主长期运行 | Agent 由用户/IM 事件触发；无后台自主 loop | M3 评估是否需要定时任务触发 |
| Memory Evolution 自动上线 | 基础设施就位（Grading / Topic Clustering / Runbook），但实际优化需人工审批后生效 | M3 实现 evolution proposal 与审批流程 |
| Playwright E2E | 前端 E2E 使用 vitest + RTL；无真实浏览器端到端测试 | M3 补充 Playwright E2E（如需要） |

## 交付物检查表

- [x] `seaki-pipeline` crate：Pipeline Designer 编译器（意图解析、类型检查、权限预估、成本估算）
- [x] `seaki-agent` crate：Agent Runtime（LLM 调用、skills 调度、session compaction、MCP 适配）
- [x] Pipe Runtime：真实执行引擎（streaming、checkpoint、tee/branch/join、resource limit、per-step policy）
- [x] MCP 兼容层：`mcp-to-pipe` 和 `pipe-to-mcp` adapter
- [x] Channel Bridge 运行时：WASM 插件运行时、Secret Broker、Ingress 归一化、Identity Mapping
- [x] 飞书插件：`plugins/channel/feishu/` protocol adapter
- [x] Quarantine 管道：远程附件下载、hash/mime 校验、malware scan stub
- [x] Outbox Dispatcher：lease-based 调度、重试、幂等调和
- [x] 自动 Memory：`MemoryItem` 状态机、自动收集器、conflict detection、frozen snapshot
- [x] Review Learning：遗忘曲线调度器、review queue、grading feedback、card generation
- [x] Topic Clustering 与 RunbookIndex：自动聚类、可执行手册索引
- [x] Electron 前端：Pipeline Designer UI、Agent Chat + Skill 选择、Memory Review、Channel 管理
- [x] E2E 测试：Happy Path + Reject Path 回归测试（Rust 44 套件 + 前端 62 用例）
- [x] M2 操作手册与架构维护记录
- [x] `cargo test --workspace` 全绿
- [x] `pnpm typecheck` / `pnpm lint` / `pnpm test` 全绿
