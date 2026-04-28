# 边界与权威链路

[返回架构索引](../architecture.md)

权威范围：Rust Core、执行链路、untrusted taint、前端边界和索引隐私边界。

## 边界范围限制

seaki 必须把“谁能决定什么”写成架构约束。边界不是代码风格问题，而是安全、可测试性和多端一致性的基础。

### Rust Core 边界

Rust Core 拥有以下职责：

- 工作区生命周期、任务生命周期、审计日志和持久化账本。
- wiki 的 source、page、claim、citation registry、wiki log 等权威状态；搜索/vector/entity index 只作为派生物管理。
- 受限记忆、会话搜索索引、复习计划和记忆审计。
- 权限裁决、opaque capability grant、approval、sandbox profile 和执行调度。
- Pipe Command Interface、MCP 兼容适配、AI skills 调度和 Channel Event 归一化。
- 所有本地文件、命令、网络、secret、IM 出站动作的最终执行权。

Rust Core 不负责：

- 具体 UI 布局、页面交互细节、视觉状态和平台控件实现。
- 第三方 IM SDK 的内部协议细节。
- 插件自己的网络登录流程和平台特有交互。
- 前端缓存策略之外的展示优化。
- 让 AI 直接控制本地文件系统或直接执行程序。

强制边界规则：

- Core 只接受结构化 DTO、pipe request、channel event 和 capability grant。
- Core 不接受未归一化的自由文本作为直接执行指令；自由文本只能作为 untrusted inert payload 存储和路由，必须经 agent proposal、deterministic validation 和 policy 后才能产生副作用。
- Core 生成的所有副作用都必须经过 `seaki-policy`。
- 插件和前端不能绕过 `seaki-daemon` 直接写入 wiki、SQLite 账本或本地文件。
- 前端、插件和 agent 不能直接写入 memory store；只能提交 `memory.propose`，由 Core 校验、压缩、去重和审计。
- 任何跨 workspace、跨 channel、跨 account 的操作都必须显式携带 scope。

### 唯一权威执行链路

所有入口都必须走同一条权威链路，不能在 Channel、前端、插件、MCP adapter 或 pipe runtime 中各自实现副作用裁决。

```text
daemon ingress
  -> authenticate / normalize
  -> create inert event
  -> agent proposes intent or plan
  -> deterministic compiler validates schema and scope
  -> policy decides
  -> sandbox or broker enforces
  -> audit / WAL / outbox records result
```

边界规则：

- Ingress 只做认证、签名校验、限流、防重放和事件归一化。
- Core 创建 inert event，不直接把事件文本转成执行。
- Agent 只能提出 intent、plan、patch 或 pipeline AST。
- Deterministic compiler 负责 schema、scope、类型、资源预算和权限预估。
- `seaki-policy` 是唯一副作用裁决点。
- `seaki-sandbox`、secret broker、channel broker 是执行约束点，不重新解释业务意图。

### Untrusted 内容与 taint 传播

所有外部输入默认是 `taint=untrusted_content`，包括 IM 文本、附件、PDF/Office/Markdown/网页内容、MCP/tool 输出、历史会话片段、搜索 snippet 和用户粘贴文本。

taint 必须随以下对象传播：`ParsedFrame`、`FrameEnvelope`、`IndexCandidate`、`SearchResultDTO`、`MemoryCandidate`、`WikiPatchProposal`、agent context 和 approval diff。未经确定性 validator 与明确 Core/用户授权，untrusted content 不能升级为：

- `PolicyDecision`
- `CapabilityGrant`
- `ToolInstruction`
- `SystemPrompt`
- `ApprovalDecision`
- 文件路径、命令、网络目标或 secret scope

agent 可以引用 tainted 内容生成 proposal，但 compiler/policy 只能读取结构化字段和已授权 scope，不能从 tainted text 中提取权限结论。

### 前端业务边界

前端业务层是跨端共享的领域 SDK，不是 UI 组件库。它负责把各端 UI 的用户动作转换为稳定的 use case 和 DTO。

前端业务层拥有以下职责：

- `source`、`wiki`、`search`、`agent run`、`approval`、`channel thread` 等 use case 编排。
- DTO 校验、请求去重、任务状态机、streaming event 合并和失败回滚。
- optimistic UI 所需的临时状态，但不把临时状态写成权威事实。
- 多端一致的错误模型、权限提示模型和审批交互模型。

前端业务层不负责：

- 判断本地路径是否安全。
- 判断命令、网络、secret 或 IM 出站动作是否允许。
- 绕过 Rust Core 直接调用 MCP、pipe command 或本地工具。
- 维护 wiki 权威索引、claim graph 或审计日志。
- 维护 memory 权威内容、复习调度或会话搜索索引。
- 把平台 UI 事件直接映射成文件系统写入。

各端 UI 边界：

- Web / Electron / React Native / 小程序 / 鸿蒙只实现界面、平台文件选择器、通知、剪贴板、分享、登录等平台能力。
- UI 端拿到本地文件只能生成 `user_selected_file` 或 `capability_grant_request`，不能自行读取并注入 agent 上下文。
- UI 端可以缓存列表和预览，但不能成为 source of truth。
- Electron 可以提供文件树和编辑器能力，但文件读写仍必须通过 `seaki-daemon` 和 `seaki-policy`。

### 索引隐私边界

向量索引、实体图和全文索引都是敏感数据副本，不能因为它们是“派生物”就放松保护。

- 默认使用本地 embedding；远程 embedding 必须单独审批、审计并显示数据范围。
- 索引按 workspace、account、channel scope 隔离，默认加密落盘。
- source tombstone 或权限收回后，索引必须同步清理或标记不可检索。
- entity graph 不能跨 workspace 自动合并身份、组织、文件或 channel 关系。
- index 是可重建派生物，不作为权威事实源。
- index 查询只能返回 candidate ids；daemon 必须按当前 actor、workspace、thread/channel scope、source status 和 citation visibility 做二次授权后，才能生成 snippet、citation refs 或 answer context。
