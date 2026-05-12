# M4 阶段任务计划：IPC 桥接 → 流式输出 → 生产可用

> 本计划按「安全修复 → 基础设施 → 能力完善 → 质量保证」顺序安排 M4 任务。详细架构事实以 `docs/architecture/` 各主题页为准。

## 阶段目标

M4 在 M3（LLM 真实调用、Citation-backed Answer、飞书消息闭环）基础上，完成 **前端 ↔ daemon 完整 IPC 桥接、流式输出、多 Provider 动态切换、飞书附件支持**，并修复架构审计中的安全与质量项，使系统达到生产可用状态：

```text
User Intent / IM Message
  -> Electron IPC / WebSocket → Daemon API Gateway
  -> LLM 调用（支持流式输出 + 多 Provider 动态切换）
  -> Citation-backed Answer 生成（增量渲染）
  -> 前端 ChatPanel 实时展示（token 流 + citation badge 可点击回跳）
  -> 飞书真实消息发送（支持文本 + 附件）
```

完成标准：
- 前端通过 IPC 与 Rust daemon 通信，不再依赖 mock transport。
- LLM 支持 SSE 流式输出到前端，token 逐字渲染。
- 支持多 LLM provider 配置与运行时动态切换。
- 飞书支持附件收发（下载经过 Quarantine，发送经过 Outbox）。
- 前端支持运行时 mock/real 模式切换，无需重启。
- 修复 M2 maintenance log 中的高危安全项（路径遍历、template injection、TOCTOU）。
- 补齐 11 个前端零测试文件的单元测试覆盖。
- 通过 Playwright 真实浏览器 E2E 覆盖至少一条 Happy Path 和一条 Reject Path。
- 所有质量门禁保持通过：660+ Rust tests、111+ 前端 tests。

## 架构依据

| 依据 | 对任务计划的约束 |
|------|----------------|
| [MVP 顺序与主要风险](../../architecture/roadmap-risks.md) | M4 交付 IPC 桥接、流式输出、多 provider、附件支持；跨平台 sandbox 和多端仍后置。 |
| [总览与核心分层](../../architecture/overview.md) | `seaki-daemon` 暴露统一 API Gateway；`seaki-agent` 负责模型调用与 session；前端通过 `@seaki/transport` 接入。 |
| [前端架构](../../architecture/frontend.md) | `@seaki/transport` 是 IPC/HTTP/WebSocket 抽象层；`@seaki/state` 管理流式事件和 replay；断线重连应能根据 `seq`、`task_id` 恢复状态。 |
| [Channel Bridge 插件化](../../architecture/channel-bridge.md) | 附件只能以 `ChannelAttachmentRef` 形式进入 Core；下载必须由 Core-owned broker HTTP client 执行；Quarantine 必须验证 `observed_mime`、`content_hash`、`malware_scan_status`。 |
| [管道命令接口](../../architecture/pipeline.md) | 支持 streaming：上游产出一条 frame，下游即可消费；`FrameEnvelope` 协议已定义。 |
| [架构维护日志](../../architecture/maintenance-log.md) | `quarantine_path` 存在路径遍历漏洞（Critical）、`substitute_vars` 存在 template injection（High）、`FakeWebhookVerifier` 存在 TOCTOU race（Medium），M4 必须修复。 |

## 非目标

- 不引入前端 ↔ daemon 的 WebSocket 传输（M4 仅实现 Electron IPC；WebSocket 为 M5 预留）。
- 不做跨平台 sandbox（Linux bubblewrap、Windows 仍后置）。
- 不做 Web/RN/小程序/Harmony 多端移植（MVP 保持 Electron-only）。
- 不实现 Agent 自主长期运行循环（触发方式仍为用户/IM 事件驱动）。
- 不修改 Memory 遗忘曲线算法本身（仅修复空 audit 实现）。
- 不引入语音/视频消息支持（飞书仅扩展文本 + 文件类附件）。

---

## 任务拆解

### P1: 安全修复（前置，无依赖）

> 前置条件：无。M4 所有后续任务建立在安全基线之上。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-S01 | 修复 quarantine 路径遍历漏洞 | `seaki-channel/src/quarantine.rs`：`sanitize_path()` + `canonicalize()` 校验 | `cargo test path_traversal_attempt_is_rejected` 通过；恶意路径返回 `QuarantineError::InvalidPath` | — |
| M4-S02 | 修复 `substitute_vars` template injection | `seaki-agent/src/dispatch.rs`：变量白名单 + 转义/拒绝策略 | `cargo test template_injection_attempt_is_rejected` 通过；`{{cmd}}` 等非法占位符被拒绝 | — |
| M4-S03 | 修复 `FakeWebhookVerifier` TOCTOU race | `seaki-channel/src/webhook.rs`：原子校验或一次性 token | `cargo test webhook_toctou_race_is_safe` 通过；并发重复验证返回已处理 | — |
| M4-S04 | 配置 WASM Engine fuel/memory limit | `seaki-channel/src/plugin/runtime.rs`：`Config::fuelConsumption(true)` + `memory_limit` | `cargo test wasm_plugin_exceeds_memory_limit_is_terminated` 通过；超限时返回 `PluginError::ResourceExhausted` | — |
| M4-S05 | 补齐 `memory_propose_pipeline` 空 audit 实现 | `seaki-memory/src/propose_pipeline.rs`：audit 日志写入 | `cargo test memory_propose_creates_audit_log` 通过；审计记录包含 `operation`、`timestamp`、`actor` | — |

### P2: IPC 基础设施（核心依赖）

> 前置条件：P1-S01~S04 完成（daemon 生命周期安全基线）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-I01 | Daemon 进程生命周期管理 | `apps/electron/src/electron/main.ts`：spawn Rust daemon、健康检查、崩溃重启 | `pnpm build` 通过；手动验证 daemon 随 Electron 启动/退出 | — |
| M4-I02 | IPC 协议封装层 | `apps/electron/src/electron/preload.ts`：`ipcRenderer.invoke` 封装；`packages/transport/src/ipc.ts` | 前端可调用 `electronAPI.sendMessage()` / `electronAPI.onEvent()` | M4-I01 |
| M4-I03 | `@seaki/transport` 真实 IPC 实现 | `packages/transport/src/ipcTransport.ts`：`createIpcTransportClient()` | 单元测试：`transport.sendRequest` 往返 < 50ms mock；类型与 mock transport 兼容 | M4-I02 |
| M4-I04 | Daemon API Gateway | `crates/seaki-daemon/src/gateway.rs`：HTTP server（`tokio::net::TcpListener`）或 IPC socket，暴露 `workspace.init`、`search.query`、`message.send` 等端点 | `cargo test gateway_echo_request` 通过；支持 request/response 和 server-sent events 两种模式 | M4-I01 |
| M4-I05 | 前端连接管理与断线重连 | `packages/state/src/connection.ts`：连接状态机（`connecting`/`connected`/`reconnecting`/`disconnected`）；指数退避重试；`seq` 恢复 | `pnpm test connection_reconnects_with_backoff` 通过；断线 3 次内自动恢复 | M4-I03 |
| M4-I06 | 前端迁移：mock → real IPC | `ChatPanel.tsx`、`appModel.ts`：替换 `createMockTransportClient()` 为 `createIpcTransportClient()`；移除 TODO 注释 | `pnpm test ChatPanel_sends_message_via_ipc` 通过；PipelinePanel TODO 全部清除 | M4-I03, M4-I05 |

### P3: 运行时配置与 mock/real 切换

> 前置条件：P2 完成（需要 IPC 传输配置变更）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-R01 | 后端配置持久化 | `crates/seaki-core/src/config_store.rs`：TOML/JSON 配置文件读写；Provider 配置结构体 | `cargo test config_store_roundtrip` 通过；配置变更持久化到 `~/.seaki/config.toml` | M4-I04 |
| M4-R02 | 配置热重载 API | `crates/seaki-daemon/src/gateway.rs` 新增 `config.get`、`config.set`、`config.reload` 端点 | `cargo test config_hot_reload_updates_active_provider` 通过；变更后无需重启 daemon | M4-R01 |
| M4-R03 | 前端运行时 mock/real 切换 UI | `apps/electron/src/components/SettingsPanel.tsx` 或 CommandPalette 命令：`toggle-mock-mode` | `pnpm test settings_panel_toggles_mock_mode` 通过；切换后立即生效，不刷新页面 | M4-I06, M4-R02 |

### P4: 多 LLM Provider 动态切换

> 前置条件：P3 完成（需要配置持久化支持多 provider 存储）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-L01 | Provider 注册表与配置存储 | `crates/seaki-agent/src/provider_registry.rs`：`LlmProviderRegistry` 持有多个命名配置；`ProviderConfig` 支持 OpenAI / Azure / Ollama / Anthropic 变体 | `cargo test provider_registry_lists_all_providers` 通过；4 种 provider 配置解析正确 | M4-R01 |
| M4-L02 | 动态 provider 切换与路由 | `crates/seaki-agent/src/provider_registry.rs`：`set_active_provider()`、`complete_with_fallback()`；`LlmRequest` 扩展 `preferred_provider` 字段 | `cargo test provider_fallback_on_rate_limit` 通过；主 provider 限流时自动切到备用 | M4-L01 |
| M4-L03 | 前端 Provider 选择器 | `ChatPanel.tsx` header 新增 provider badge + dropdown；`WikiSidebar` Settings tab 新增 provider 管理 | `pnpm test provider_selector_switches_active_provider` 通过；切换后新请求使用新 provider | M4-I06, M4-L02 |

### P5: LLM 流式输出

> 前置条件：P2 完成（需要 IPC 传输流数据）；P4 可选（流式与多 provider 正交）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-ST01 | `LlmClient` stream trait 扩展 | `crates/seaki-agent/src/llm.rs`：新增 `stream(&self, request) -> Result<LlmStream, LlmError>`；`LlmStream` 为 `Stream<Item = LlmChunk>` | `cargo test stream_yields_token_chunks` 通过；mock provider 也能产出 chunk | — |
| M4-ST02 | async-openai SSE 流式解析 | `crates/seaki-agent/src/openai_stream.rs`：SSE parser、chunk 提取、错误映射 | `cargo test openai_stream_parses_delta_content` 通过；覆盖 `data: [DONE]` 终止 | M4-ST01 |
| M4-ST03 | Daemon 流式事件推送 | `crates/seaki-daemon/src/gateway.rs`：Server-Sent Events endpoint；`FrontendEventEnvelope` 扩展 `StreamChunk` 变体 | `cargo test gateway_stream_events_are_ordered` 通过；chunk 按 `seq` 顺序到达 | M4-I04, M4-ST02 |
| M4-ST04 | 前端流式渲染 | `packages/state/src/streaming.ts`：流式 reducer；`ChatPanel.tsx`：token 逐字追加渲染；打字机效果 | `pnpm test streaming_renders_tokens_incrementally` 通过；100 个 chunk 在 200ms 内渲染完成 | M4-I06, M4-ST03 |
| M4-ST05 | `AnswerComposer` 增量 citation 处理 | `crates/seaki-agent/src/compose.rs`：流式完成后统一提取 citation；或定义增量提取策略（流中无 `[N]` 时纯文本，完成后解析） | `cargo test compose_stream_deferred_citation_extraction` 通过；流式回答最终包含正确 `[N]` | M4-ST01 |

### P6: 飞书附件支持

> 前置条件：P1-S01 完成（quarantine 安全修复）；P2 完成（IPC 传输附件元数据）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-F01 | Feishu 消息解析（附件元数据提取） | `crates/seaki-channel/src/feishu_ingress.rs`：解析 `message` 事件中的 `file_key`、`file_name`、`mime_type`、`size`；产出 `ChannelAttachmentRef` | `cargo test feishu_parse_message_with_attachment` 通过；文本消息行为不变 | M4-S01 |
| M4-F02 | 文件下载与 Quarantine 集成 | `crates/seaki-channel/src/feishu_download.rs`：调用 Feishu Drive API 下载；经 Quarantine 管道（hash、mime、malware scan） | `cargo test feishu_download_quarantines_file` 通过；恶意文件返回 `QuarantineError::Rejected` | M4-F01 |
| M4-F03 | Secret Broker 扩展（drive 权限） | `crates/seaki-channel/src/broker/secret.rs`：新增 `feishu.drive:file.read` scope；token 申请时包含 drive 权限 | `cargo test broker_issues_drive_scoped_token` 通过；scope 包含 `drive:file.read` | M4-F02 |
| M4-F04 | Outbox 附件发送流程 | `crates/seaki-channel/src/feishu_http.rs`：扩展 `send()` 支持 `msg_type: "file"` / `"image"`；先 upload 获取 `file_key`，再 send message | `cargo test feishu_sends_file_message` 通过；附件消息走 Outbox 调度 | M4-F03 |
| M4-F05 | 前端附件展示 | `apps/electron/src/components/ChannelPanel.tsx`：附件列表渲染；下载状态 badge；点击预览（调用系统默认程序） | `pnpm test channel_panel_shows_attachment_list` 通过 | M4-I06, M4-F04 |

### P7: 测试覆盖补齐

> 前置条件：P2~P6 完成（新功能测试与旧文件补齐并行进行）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-T01 | 11 个前端零测试文件补齐 | 为以下文件创建/补充测试：具体文件列表见 `docs/architecture/maintenance-log.md` | `pnpm test --coverage` 中这些文件的覆盖率 > 70% | — |
| M4-T02 | Rust 安全修复回归测试 | P1 所有安全修复对应的单元测试、模糊测试或 property-based test | `cargo test --workspace` 中新增 15+ 安全相关测试通过 | P1 |
| M4-T03 | IPC 桥接集成测试 | `apps/electron/e2e/ipc-bridge.spec.ts`：Electron main + renderer 进程间通信端到端测试 | `pnpm e2e ipc-bridge` 通过（或 `pnpm test` 中的集成测试） | P2 |

### P8: Playwright E2E 端到端测试

> 前置条件：P2~P6 完成（E2E 覆盖真实功能链路）。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M4-E01 | Playwright E2E 基础设施 | `apps/electron/e2e/`：Playwright 配置、fixture、helpers、CI 集成 | `pnpm e2e` 命令可运行；Playwright 报告生成 | P2 |
| M4-E02 | Happy Path E2E | `e2e/happy-path.spec.ts`：启动 App → 发送消息 → LLM 生成回答（mock 或 real）→ citation badge 可点击 → 触发飞书发送 | 完整链路在 30s 内跑完，无人工干预 | P2, P3, P5, P6 |
| M4-E03 | Reject Path E2E | `e2e/reject-path.spec.ts`：LLM 不可用时的降级、飞书 token 过期重试、IPC 断线重连 | 3 个拒绝场景各自独立通过 | P2, P5, P6 |

---

## 推荐执行顺序

```
P1 (安全修复)
  |
  +-- P2 (IPC 基础设施) + P7-T01 (前端测试补齐，可并行)
       |
       +-- P3 (运行时配置)
            |
            +-- P4 (多 Provider) + P5 (流式输出) + P6 (飞书附件) [三者并行]
                 |
                 +-- P7-T02/T03 (Rust 回归 + IPC 集成测试)
                      |
                      +-- P8 (Playwright E2E)
```

### 关键路径（无并行优化时）

```
P1 → P2 → P3 → P4 → P7 → P8   (约 6 个串行阶段)
     ↘    ↓    ↘
          P5 → ┘
          P6 → ┘
```

### 可并行加速点

- **P1 与 P7-T01 并行**：安全修复和前端测试补齐互不依赖。
- **P4 / P5 / P6 并行**：多 provider、流式输出、飞书附件三个方向正交，团队可分组推进。
- **P2 中 I01~I04 与 P1 并行**：daemon 生命周期和 gateway 的开发与前端安全修复可同步进行。

---

## 质量门禁

最小门禁（与 M0/M1/M2/M3 保持一致）：

- Rust：`cargo fmt --check`、`cargo clippy --workspace --tests -- -D warnings`、`cargo test --workspace`。
- TypeScript/Electron：`pnpm typecheck`、`pnpm lint`、`pnpm test`。
- DTO：`pnpm dto:check`（如修改 Rust DTO）。
- 文档：新增或修改 Markdown 后检查相对链接。

M4 新增关键回归测试：

- `quarantine_path` 拒绝路径遍历尝试（`../etc/passwd` 等）。
- `substitute_vars` 拒绝非法占位符（`{{cmd}}`、`{{env.PATH}}` 等）。
- `FakeWebhookVerifier` 并发重复验证安全。
- WASM plugin 超内存限制被终止而非 panic。
- `ipcTransport.sendRequest` 往返延迟 < 50ms（本地 IPC）。
- `connection` 断线后 3 次内自动恢复。
- `provider_registry` 主 provider 限流时 fallback 到备用。
- `stream` mock provider 产出有序 chunk。
- `feishu_download` 恶意文件被 Quarantine 拒绝。
- ChatPanel 流式渲染 100 个 chunk 在 200ms 内完成。
- Playwright E2E Happy Path 30s 内无人工干预通过。

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| IPC 桥接引入后前端测试大量失效（mock transport 被替换） | 渐进迁移：保留 mock transport 作为 fallback，通过配置切换；所有现有测试先跑 mock 模式，新增 IPC 测试独立运行。 |
| 流式输出与 citation 解析冲突（流中 `[N]` 可能截断） | 流式阶段不解析 citation，全部内容到达后统一提取；前端先渲染纯文本，完成后高亮 citation badge。 |
| 多 provider 配置格式不兼容（OpenAI vs Azure vs Anthropic） | 定义统一的 `ProviderConfig` 结构体，各 provider 的差异在 `convert_request` 中处理；新增 provider 时向后兼容。 |
| 飞书附件下载大文件导致内存溢出 | Quarantine 中设置 `max_file_size` 限制（如 50MB）；大文件返回 `QuarantineError::TooLarge`；流式下载写入临时文件而非内存。 |
| Electron + Rust daemon 进程生命周期管理复杂（崩溃、僵尸进程） | main process 中注册 `app.on('before-quit')` 清理 daemon；daemon 侧实现 PID 文件和心跳超时自退。 |
| Playwright E2E 在 CI 中不稳定（Electron 启动慢、时序问题） | 使用 `test.slow()` 标记；增加重试机制（`retries: 2`）；CI 使用 `xvfb` 或无头模式。 |

---

## 交付物清单

- `seaki-daemon` crate：API Gateway（HTTP server）、进程生命周期管理、配置热重载端点。
- `seaki-agent` crate：`LlmClient::stream()`、`OpenAiClient` SSE 解析、`LlmProviderRegistry`、动态切换与 fallback。
- `seaki-channel` crate：Feishu 附件消息解析、下载与 Quarantine 集成、Secret Broker drive scope、Outbox 附件发送。
- `seaki-core` crate：配置持久化存储（`config_store.rs`）。
- 前端 `@seaki/transport`：真实 IPC transport 实现。
- 前端 `@seaki/state`：连接状态机、流式事件 reducer、断线重连。
- 前端组件：`ChatPanel` provider 选择器、流式渲染；`ChannelPanel` 附件列表；Settings mock/real 切换。
- 安全修复：`quarantine.rs` 路径遍历修复、`dispatch.rs` template injection 修复、`webhook.rs` TOCTOU 修复、`plugin/runtime.rs` WASM 限制配置、`propose_pipeline.rs` audit 补齐。
- 测试：11 个前端文件补齐测试、15+ Rust 安全回归测试、IPC 集成测试、Playwright E2E（Happy Path + Reject Path）。
- 文档：`m4-task-plan.md`（本文件）、`m4-operation-manual.md`（验收手册）。

---

## 暂缓到后续阶段（M5+）

- 前端 ↔ daemon WebSocket 传输（当前仅 Electron IPC）。
- 跨平台 sandbox（Linux bubblewrap、Windows）。
- Web/React Native/小程序/Harmony 多端移植。
- Agent 自主长期运行循环（非事件触发）。
- Memory evolution 自动上线（仍需人工 approval workflow）。
- 语音/视频消息支持。
- 飞书群机器人/自定义机器人模式。
