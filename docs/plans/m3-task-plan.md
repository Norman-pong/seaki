# M3 阶段任务计划：LLM 接入 → Citation-backed Answer → 飞书闭环

> 本计划按「基础设施 → 后端实现 → 前端接入 → 端到端验证」顺序安排 M3 任务。详细架构事实以 `docs/architecture/` 各主题页为准。

## 阶段目标

M3 在 M2（Pipeline / Agent / Channel / Memory 纵切）基础上，将骨架替换为真实实现，完成 **LLM 接入 → Citation-backed Answer → 飞书真实消息闭环**：

```text
User Intent / IM Message
  -> LLM 调用（async-openai 驱动）
  -> Citation-backed Answer 生成（[1][2] 标记 + 来源映射）
  -> 前端 ChatPanel 展示（citation badge 可点击回跳）
  -> 飞书真实消息发送（HTTP ProviderDriver）
```

完成标准：
- `OpenAiClient` 能通过 async-openai 调用真实 LLM API（OpenAI / Azure / Ollama / vLLM 兼容）。
- `AnswerComposer` 能用 LLM 生成带 `[1][2]` citation 标记的自然语言回答。
- `FeishuProviderDriver` 能调用真实飞书 Open Platform API 发送消息。
- 前端 ChatPanel citation badge 可点击，调用 `citation.resolve` 回跳到 source range 或 wiki anchor。
- 通过环境变量 `SEAKI_LLM_ENABLED` 控制 mock/real 模式切换，未配置时保持 mock 行为。
- 所有关键路径有测试覆盖，拒绝路径有回归测试。

## 架构依据

| 依据 | 对任务计划的约束 |
|------|----------------|
| [MVP 顺序与主要风险](../../architecture/roadmap-risks.md) | M3 交付真实 LLM 调用、citation-backed answer、飞书真实消息收发；前端 IPC 桥接和多端仍后置。 |
| [总览与核心分层](../../architecture/overview.md) | `seaki-agent` 引入 tokio + async-openai；`seaki-channel` 引入 reqwest；保持同步 trait 接口不变。 |
| [边界与权威链路](../../architecture/boundaries.md) | LLM 输出只作为 proposal；citation 必须经过 `citation.resolve` 验证后才能回跳；飞书消息必须经过 Outbox 调度器。 |
| [管道命令接口](../../architecture/pipeline.md) | MCP 适配层输出带 `taint=untrusted_content`；外部 tool 的 schema 自动转换为 command manifest。 |
| [Rust Sandbox Runtime](../../architecture/sandbox-runtime.md) | 不新增平台后端；继续使用现有 macOS Seatbelt 抽象。 |
| [Channel Bridge 插件化](../../architecture/channel-bridge.md) | 飞书适配器只做协议适配，不持有 secret；HTTP 调用经 ProviderDriver 和 Outbox 调度器。 |

## 非目标

- 不引入流式输出（streaming）到前端；M3 保持同步 complete() 调用。
- 不实现前端 ↔ daemon 的完整 IPC 桥接；ChatPanel 通过环境变量切换 mock/real。
- 不做多 LLM provider 同时切换；一次只配置一个 provider（通过 `SEAKI_LLM_API_BASE`）。
- 不实现飞书附件上传；仅实现消息发送。
- 不修改 seaki-core 的 `compose_answer`（snippet 拼接保留为 fallback）。

---

## 任务拆解

### P1: tokio Runtime 基础设施

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| 引入 tokio + async-openai 依赖 | `crates/seaki-agent/Cargo.toml` 新增依赖 | `cargo test -p seaki-agent` 通过 |
| 实现 AgentRuntimeHandle | `runtime_handle.rs`：封装 tokio runtime，提供 `block_on()` | 7 个测试覆盖创建、执行、clone、复用场景 |

### P2: OpenAiClient 真实实现

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| 改造 OpenAiClient | `llm.rs`：真实 HTTP 调用 | `complete()` 不再返回 `ModelUnavailable` |
| OpenAiClientConfig | 环境变量配置（`SEAKI_LLM_API_BASE`/`KEY`/`MODEL`） | 配置读取测试通过 |
| 请求/响应转换 | `convert_request()` / `convert_response()` | 4 种 MessageRole 全部映射正确 |
| 错误映射 | `map_openai_error()` | 覆盖 ApiError / RateLimited / 网络错误 |

### P3: LLM 驱动 compose_answer

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| AnswerComposer | `compose.rs`：LLM 生成 citation-backed answer | 给定 search result 输出含 `[N]` 标记 |
| Citation 标记提取 | 简单字符解析器 | 去重、排序、越界过滤 |
| 降级路径 | composed / degraded / fallback 三态 | 无 LLM 时 fallback，无 citation 时 degraded |

### P4: 飞书 HTTP ProviderDriver

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| 引入 tokio + reqwest | `crates/seaki-channel/Cargo.toml` 新增依赖 | `cargo test -p seaki-channel` 通过 |
| FeishuProviderDriver | `feishu_http.rs`：实现 `ProviderDriver` trait | `send()` 能构造正确的 HTTP 请求 |
| AccessToken 管理 | 缓存 + 过期刷新 + 401 重试 | token 过期自动刷新 |
| 错误映射 | 飞书 code → ProviderError | 覆盖 99991400 / 99991401 / 网络错误 |

### P5: 前端 ChatPanel 接入

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| CitationRef 类型扩展 | `chatModel.ts`：完整字段 | 向后兼容（新字段 optional） |
| Citation badge 可点击 | `ChatPanel.tsx`：button + onClick | 2 个测试验证点击和回调 |
| LLM 模式控制 | `SEAKI_LLM_ENABLED` 环境变量 | 未配置时保持 mock |
| CommandPalette 扩展 | `compose-answer` 命令 | Cmd+Shift+A 快捷键 |

### P6: 端到端验证

| 任务 | 主要产出 | 验收标准 |
|------|----------|----------|
| 全量 Rust 测试 | `cargo test --workspace` | 660+ tests pass |
| 全量前端测试 | `pnpm test` | 111 tests pass |
| 质量门禁 | fmt / clippy / typecheck / lint / dto:check | 全部通过 |

---

## 推荐执行顺序

```
P1 (tokio 基础)
  |
  +-- P2 (OpenAiClient) + P4 (飞书 HTTP) [并行]
       |
       +-- P3 (compose_answer)
            |
            +-- P5 (前端接入)
                 |
                 +-- P6 (集成验证)
```

---

## 质量门禁

最小门禁（与 M0/M1/M2 保持一致）：

- Rust：`cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。
- TypeScript/Electron：`pnpm typecheck`、`pnpm lint`、`pnpm test`。
- DTO：`pnpm dto:check`（如修改 Rust DTO）。
- 文档：新增或修改 Markdown 后检查相对链接。

M3 新增关键回归测试：

- `AgentRuntimeHandle::block_on()` 不 panic（含复用已有 runtime 场景）。
- `OpenAiClient::complete()` 网络错误返回 `LlmError::RequestFailed` 非 panic。
- `AnswerComposer::compose()` citation 标记提取失败降级为纯文本 answer。
- `FeishuProviderDriver::send()` token 过期自动刷新并重试一次。
- ChatPanel citation badge 点击触发 `onCitationClick` 回调。

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| `block_on` 嵌套 panic | `AgentRuntimeHandle` 优先 `Handle::try_current()` 复用 runtime。 |
| async-openai 版本冲突 | workspace dependencies 统一管理；锁定 `0.38` 版本。 |
| LLM 输出格式不符合预期 | 防御性解析：citation 提取失败返回纯文本。 |
| 飞书 token 过期 | 缓存 + 自动刷新 + 401 重试一次。 |
| 前端 mock/real 切换冲突 | 环境变量 `SEAKI_LLM_ENABLED` 控制。 |

---

## 交付物清单

- `seaki-agent` crate：tokio runtime 基础设施、`AgentRuntimeHandle`、`OpenAiClient` 真实实现、`AnswerComposer` LLM answer 生成器。
- `seaki-channel` crate：`FeishuProviderDriver` 真实飞书 HTTP 调用、tenant_access_token 管理。
- 前端：CitationRef 完整类型、citation badge 可点击、`SEAKI_LLM_ENABLED` 环境变量控制、`compose-answer` 命令。
- 测试：7 个新增 runtime_handle 测试、7 个新增 OpenAiClient 测试、5 个新增 compose 测试、2 个新增前端测试。
- 质量门禁：全量 660+ Rust tests、111 前端 tests 通过。

---

## 暂缓到后续阶段（M4+）

- 前端 ↔ daemon 完整 IPC 桥接。
- LLM 流式输出（streaming）到前端。
- 多 LLM provider 动态切换。
- 飞书附件上传。
- Playwright 真实浏览器 E2E。
