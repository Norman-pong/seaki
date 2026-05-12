# M3 端到端验收与发布门禁操作手册

[返回任务计划](m3-task-plan.md)

本手册记录 M3 端到端验收步骤、质量门禁命令和已知限制清单。M3 交付范围覆盖 LLM 真实调用（async-openai）、LLM 驱动 citation-backed answer 生成、飞书真实 HTTP 消息发送，以及前端 citation 回跳交互。

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

## M3 关键测试运行

### tokio Runtime 基础设施

```bash
cargo test -p seaki-agent runtime_handle -- --nocapture
```

### OpenAiClient 真实调用

```bash
# 单元测试（MockLlmClient）
cargo test -p seaki-agent openai -- --nocapture

# 集成测试（需要 Ollama 运行）
cargo test -p seaki-agent openai_client_integration -- --ignored --nocapture
```

### LLM compose_answer

```bash
cargo test -p seaki-agent compose -- --nocapture
```

### 飞书 ProviderDriver

```bash
# 单元测试
cargo test -p seaki-channel feishu_http -- --nocapture

# 集成测试（需要飞书测试 app 凭据）
cargo test -p seaki-channel feishu_sends -- --ignored --nocapture
```

### 前端组件测试

```bash
# Citation badge 可点击性
pnpm test -- --run ChatPanel

# CommandPalette compose-answer 命令
pnpm test -- --run CommandPalette
```

## Happy Path 演示

### 1. LLM 真实调用

```rust
// crates/seaki-agent/src/llm.rs
let config = OpenAiClientConfig::from_env().unwrap();
let client = OpenAiClient::new(config, AgentRuntimeHandle::new());

let request = LlmRequest {
    model: "gpt-4o-mini".to_string(),
    messages: vec![
        LlmMessage { role: MessageRole::System, content: "You are a helpful assistant.".to_string(), name: None },
        LlmMessage { role: MessageRole::User, content: "Hello".to_string(), name: None },
    ],
    temperature: None,
    max_tokens: None,
};

let response = client.complete(request).unwrap();
assert!(!response.content.is_empty());
```

手动验证（需 Ollama）：
```bash
cargo test -p seaki-agent openai_client_integration -- --ignored --nocapture
```

### 2. LLM Citation-backed Answer

```rust
// crates/seaki-agent/src/compose.rs
let composer = AnswerComposer::new(Box::new(client));
let result = composer.compose(ComposeRequest {
    query: "What is Rust ownership?".to_string(),
    search_results: vec![
        SearchContextItem {
            title: "Rust Book".to_string(),
            snippet: "Ownership is Rust's most unique feature.".to_string(),
            citation_id: "c1".to_string(),
            source_id: "src1".to_string(),
        },
    ],
    workspace_id: "ws1".to_string(),
}).unwrap();

assert!(result.text.contains("[1]"));
assert_eq!(result.citation_refs[0].citation_id, "c1");
```

手动验证：
```bash
cargo test -p seaki-agent compose_with_mock_llm_returns_answer -- --nocapture
```

### 3. 飞书真实消息发送

```rust
// crates/seaki-channel/src/feishu_http.rs
let config = FeishuProviderConfig::from_env().unwrap();
let driver = FeishuProviderDriver::new(config).unwrap();

let item = OutboxItem::new(
    "test-id",
    serde_json::json!({
        "receive_id": "oc_xxx",
        "receive_id_type": "chat_id",
        "content": "{\"text\":\"Hello from seaki\"}"
    }),
);

driver.send(&item).unwrap();
```

手动验证（需飞书测试 app）：
```bash
cargo test -p seaki-channel feishu_sends_message -- --ignored --nocapture
```

### 4. 前端 Citation 回跳

```bash
# Citation badge 渲染为可点击按钮
pnpm test -- --run "renders citation badges as clickable buttons"

# 点击触发回调
pnpm test -- --run "calls onCitationClick when citation badge clicked"
```

## Reject Path 回归测试

### LLM 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| API 不可达 | `cargo test openai_client_complete_returns_error_without_server -- --nocapture` | `RequestFailed` 非 panic |
| 空 API key | `cargo test openai_config_from_env_without_key_returns_none -- --nocapture` | `from_env()` 返回 `None` |

### Compose 拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| 空搜索结果 | `cargo test compose_with_empty_results_returns_fallback -- --nocapture` | `status = "fallback"` |
| 无 citation 标记 | `cargo test compose_handles_no_citation_markers -- --nocapture` | `status = "degraded"` |
| 越界 citation | `cargo test compose_handles_out_of_range_citation -- --nocapture` | 越界标记被忽略 |

### 飞书拒绝路径

| 场景 | 测试命令 | 预期结果 |
|------|----------|----------|
| Token 过期 | `cargo test token_refresh_on_expired -- --nocapture` | 自动刷新并重试 |
| 频率限制 | `cargo test rate_limited_returns_error -- --nocapture` | `ProviderError::RateLimited` |

## 已知限制和 M4 前置依赖

| 限制/依赖 | 说明 | M4 计划 |
|---|---|---|
| 前端 IPC 桥接 | ChatPanel 使用 mock transport；真实场景需通过 Electron IPC 与 Rust daemon 通信 | M4 实现前端 ↔ daemon 完整 IPC 桥接 |
| LLM 流式输出 | `complete()` 是同步调用；无 streaming 到前端 | M4 评估是否需要 SSE 流式输出 |
| 多 LLM provider | 一次只配置一个 provider | M4 支持动态切换 |
| 飞书附件上传 | 仅实现文本消息发送 | M4 按需扩展 |
| 前端 mock/real 切换 | 通过环境变量控制，非运行时切换 | M4 支持运行时切换 |

## 交付物检查表

- [x] `seaki-agent` crate：tokio runtime 基础设施（`AgentRuntimeHandle`）
- [x] `seaki-agent` crate：`OpenAiClient` 真实 LLM 调用（async-openai 驱动）
- [x] `seaki-agent` crate：`AnswerComposer` LLM citation-backed answer 生成器
- [x] `seaki-channel` crate：`FeishuProviderDriver` 真实飞书 HTTP 调用
- [x] 前端：`CitationRef` 完整类型、citation badge 可点击
- [x] 前端：`SEAKI_LLM_ENABLED` 环境变量控制 mock/real 模式
- [x] 前端：`CommandPalette` `compose-answer` 命令
- [x] 测试：全量 660+ Rust tests pass
- [x] 测试：111 前端 tests pass
- [x] M3 任务计划与操作手册
