# M4 端到端验收与发布门禁操作手册

[返回任务计划](m4-task-plan.md)

本手册记录 M4 端到端验收步骤、质量门禁命令和已知限制清单。M4 交付范围覆盖 IPC 桥接、LLM 流式输出、多 Provider 动态切换、飞书附件支持、运行时 mock/real 切换，以及安全修复与测试覆盖补齐。

## 环境准备

### LLM Provider 配置

```bash
# 方式一：OpenAI 官方 API
export SEAKI_LLM_API_BASE="https://api.openai.com/v1"
export SEAKI_LLM_API_KEY="sk-..."
export SEAKI_LLM_MODEL="gpt-4o-mini"

# 方式二：本地 Ollama
export SEAKI_LLM_API_BASE="http://localhost:11434/v1"
export SEAKI_LLM_API_KEY="ollama"
export SEAKI_LLM_MODEL="llama3"

# 方式三：Azure OpenAI
export SEAKI_LLM_API_BASE="https://your-resource.openai.azure.com/openai/deployments/your-deployment"
export SEAKI_LLM_API_KEY="..."
export SEAKI_LLM_MODEL="gpt-4o"
```

### 飞书测试应用配置

```bash
export SEAKI_FEISHU_APP_ID="cli_..."
export SEAKI_FEISHU_APP_SECRET="..."
```

### 前端 LLM 模式

```bash
# 启用真实 LLM 模式（默认 mock）
export SEAKI_LLM_ENABLED="true"
```

### M4 新增配置

```bash
# Daemon 数据目录（自动创建）
export SEAKI_DATA_DIR="$HOME/.seaki"

# 配置持久化文件路径（自动创建）
export SEAKI_CONFIG_PATH="$HOME/.seaki/config.toml"

# 多 Provider 配置示例（通过运行时 UI 或 config API 设置）
# primary_provider: openai
# fallback_provider: ollama
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

# Playwright E2E（M4 新增）
pnpm e2e
```

## M4 关键测试运行

### P1: 安全修复

```bash
# 路径遍历防护
cargo test -p seaki-channel path_traversal -- --nocapture

# Template injection 防护
cargo test -p seaki-agent template_injection -- --nocapture

# TOCTOU race 安全
cargo test -p seaki-channel webhook_toctou -- --nocapture

# WASM 资源限制
cargo test -p seaki-channel wasm_plugin_exceeds_memory -- --nocapture

# Memory audit 补齐
cargo test -p seaki-memory memory_propose_creates_audit -- --nocapture
```

### P2: IPC 基础设施

```bash
# Daemon 进程生命周期
cargo test -p seaki-daemon daemon_lifecycle -- --nocapture

# IPC transport
cargo test -p seaki-daemon gateway_echo -- --nocapture

# 前端 transport 实现
pnpm test -- --run "ipc transport sends request"

# 连接管理与断线重连
pnpm test -- --run "connection reconnects with backoff"

# 前端迁移 mock -> real
pnpm test -- --run "ChatPanel sends message via ipc"
```

### P3: 运行时配置

```bash
# 配置持久化
cargo test -p seaki-core config_store_roundtrip -- --nocapture

# 热重载
cargo test -p seaki-daemon config_hot_reload -- --nocapture

# 前端运行时切换
pnpm test -- --run "settings panel toggles mock mode"
```

### P4: 多 Provider

```bash
# Provider 注册表
cargo test -p seaki-agent provider_registry -- --nocapture

# Fallback 切换
cargo test -p seaki-agent provider_fallback -- --nocapture

# 前端 Provider 选择器
pnpm test -- --run "provider selector switches active provider"
```

### P5: 流式输出

```bash
# Stream trait
cargo test -p seaki-agent stream_yields -- --nocapture

# SSE 解析
cargo test -p seaki-agent openai_stream_parses -- --nocapture

# 流式事件有序到达
cargo test -p seaki-daemon gateway_stream_events -- --nocapture

# 前端流式渲染
pnpm test -- --run "streaming renders tokens incrementally"

# 增量 citation 提取
cargo test -p seaki-agent compose_stream_deferred -- --nocapture
```

### P6: 飞书附件

```bash
# 消息附件解析
cargo test -p seaki-channel feishu_parse_message_with_attachment -- --nocapture

# 下载与 Quarantine
cargo test -p seaki-channel feishu_download_quarantines -- --nocapture

# Secret Broker drive scope
cargo test -p seaki-channel broker_issues_drive -- --nocapture

# 附件发送
cargo test -p seaki-channel feishu_sends_file -- --nocapture

# 前端附件展示
pnpm test -- --run "channel panel shows attachment list"
```

### P7: 测试覆盖

```bash
# 前端零测试文件补齐
pnpm test --coverage

# Rust 安全回归测试
cargo test --workspace -- security_regression

# IPC 集成测试
pnpm test -- --run "ipc integration"
```

### P8: Playwright E2E

```bash
# E2E 基础设施
pnpm e2e --list

# Happy Path
pnpm e2e happy-path

# Reject Path
pnpm e2e reject-path
```

## Happy Path 演示

### 1. IPC 桥接：前端发送消息到 Daemon

```typescript
// 前端：通过 IPC transport 发送消息
import { createIpcTransportClient } from "@seaki/transport";

const transport = createIpcTransportClient();
const response = await transport.request("message.send", {
  sessionId: "sess_123",
  content: "Hello from frontend",
  skill: "wiki-search",
});
console.assert(response.messageId !== undefined);
```

手动验证：
```bash
# 1. 启动 Electron App（自动启动 daemon）
pnpm dev

# 2. 在 ChatPanel 输入消息，确认消息通过 IPC 到达 daemon
#    观察 Electron 开发者工具 Network/Console 面板无 mock transport 日志
```

### 2. LLM 流式输出

```rust
// 后端：流式调用
// crates/seaki-agent/src/llm.rs
let client = OpenAiClient::new(config, handle);
let mut stream = client.stream(request)?;

while let Some(chunk) = stream.next().await {
    match chunk {
        LlmChunk::Token(text) => println!("token: {}", text),
        LlmChunk::Done => break,
        LlmChunk::Error(e) => eprintln!("stream error: {}", e),
    }
}
```

手动验证：
```bash
# 需要 Ollama 运行
cargo test -p seaki-agent stream_integration -- --ignored --nocapture
```

### 3. 多 Provider 动态切换

```typescript
// 前端：切换 Provider
// ChatPanel header dropdown
await domainClient.llm.setActiveProvider({ providerId: "ollama-local" });

// 后端：配置持久化
// crates/seaki-core/src/config_store.rs
let config = ConfigStore::load()?;
config.set_active_provider("ollama-local");
config.save()?;
```

手动验证：
```bash
cargo test -p seaki-agent provider_integration -- --ignored --nocapture
```

### 4. 飞书附件收发

```rust
// 接收附件（经 Quarantine）
// crates/seaki-channel/src/feishu_download.rs
let result = quarantine_pipeline.process(&attachment_ref)?;
assert!(matches!(result, QuarantineResult::Clean(_)));

// 发送附件
// crates/seaki-channel/src/feishu_http.rs
let item = OutboxItem::new("test-id", json!({
    "receive_id": "oc_xxx",
    "receive_id_type": "chat_id",
    "msg_type": "file",
    "content": json!({ "file_key": "file_xxx" })
}));
driver.send(&item)?;
```

手动验证（需飞书测试 app）：
```bash
cargo test -p seaki-channel feishu_attachment_integration -- --ignored --nocapture
```

### 5. 前端运行时 mock/real 切换

```typescript
// CommandPalette: toggle-mock-mode
// 或 SettingsPanel 开关
const isMock = await domainClient.config.get("mode");
await domainClient.config.set("mode", isMock === "mock" ? "real" : "mock");
// 切换后立即生效，无需刷新页面
```

手动验证：
```bash
pnpm test -- --run "settings panel toggles mock mode"
```

## Reject Path 回归测试

### IPC 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| Daemon 未启动 | `cargo test ipc_daemon_unavailable -- --nocapture` | `DaemonUnavailableError`，UI 进入只读或重连 |
| IPC 超时 | `cargo test ipc_request_timeout -- --nocapture` | 返回超时错误，不阻塞 UI |

### 流式输出拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| SSE 解析错误 | `cargo test stream_handles_malformed_sse -- --nocapture` | 返回 `ParseFailed`，已渲染内容保留 |
| 连接中断 | `cargo test stream_recovers_from_disconnect -- --nocapture` | 断线后重连，恢复流式输出 |

### 多 Provider 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 主 Provider 不可用 | `cargo test provider_fallback_on_unavailable -- --nocapture` | 自动切换到备用 Provider |
| 所有 Provider 失败 | `cargo test provider_all_failed_returns_error -- --nocapture` | 返回 `ModelUnavailable`，非 panic |

### 飞书附件拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 恶意文件路径 | `cargo test quarantine_rejects_path_traversal -- --nocapture` | `QuarantineError::InvalidPath` |
| 文件过大 | `cargo test quarantine_rejects_oversized_file -- --nocapture` | `QuarantineError::TooLarge` |
| Token 过期（附件下载） | `cargo test token_refresh_on_drive_expired -- --nocapture` | 自动刷新并重试一次 |

### 安全修复回归测试

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 路径遍历攻击 | `cargo test path_traversal_attempt_is_rejected -- --nocapture` | `QuarantineError::InvalidPath` |
| Template injection | `cargo test template_injection_attempt_is_rejected -- --nocapture` | 非法占位符被拒绝 |
| Webhook TOCTOU | `cargo test webhook_toctou_race_is_safe -- --nocapture` | 并发重复验证安全 |
| WASM 超内存 | `cargo test wasm_plugin_exceeds_memory_limit_is_terminated -- --nocapture` | `PluginError::ResourceExhausted` |

## 已知限制和 M5 前置依赖

| 限制/依赖 | 说明 | M5 计划 |
|---|---|---|
| WebSocket 传输 | M4 仅实现 Electron IPC（ipcMain/ipcRenderer）；Web 端需要 WebSocket | M5 实现 WebSocket 传输层 |
| 跨平台 sandbox | 仅 macOS Seatbelt；Linux/Windows 未实现 | M5 引入 Linux bubblewrap、Windows sandbox |
| 多端移植 | Electron-only；Web/RN/小程序/Harmony 未实现 | M5+ 按优先级移植 |
| Agent 自主循环 | 仅用户/IM 事件触发；无自主长期运行 | M5 评估是否需要定时触发器 |
| Memory evolution 自动上线 | 基础设施存在，但仍需人工 approval | M5 实现自动 approval workflow |
| 语音/视频消息 | 飞书仅支持文本 + 文件类附件 | M5 按需评估 |

## 交付物检查表

- [ ] `seaki-daemon` crate：API Gateway（HTTP server + SSE endpoint）、进程生命周期管理
- [ ] `seaki-agent` crate：`LlmClient::stream()`、`OpenAiClient` SSE 解析、`LlmProviderRegistry`、动态切换与 fallback
- [ ] `seaki-channel` crate：Feishu 附件消息解析、下载与 Quarantine 集成、Secret Broker drive scope、Outbox 附件发送
- [ ] `seaki-core` crate：配置持久化存储（`config_store.rs`）
- [ ] 安全修复：`quarantine.rs` 路径遍历修复、`dispatch.rs` template injection 修复、`webhook.rs` TOCTOU 修复、`plugin/runtime.rs` WASM 限制配置、`propose_pipeline.rs` audit 补齐
- [ ] 前端 `@seaki/transport`：真实 IPC transport 实现
- [ ] 前端 `@seaki/state`：连接状态机、流式事件 reducer、断线重连
- [ ] 前端组件：`ChatPanel` provider 选择器、流式渲染；`ChannelPanel` 附件列表；Settings mock/real 切换
- [ ] 测试：11 个前端文件补齐测试、15+ Rust 安全回归测试、IPC 集成测试
- [ ] Playwright E2E：Happy Path + Reject Path 通过
- [ ] M4 任务计划与操作手册
