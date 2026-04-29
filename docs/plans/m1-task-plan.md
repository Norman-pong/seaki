# M1 任务计划：Pipeline · Memory · Fake Channel Bridge

[返回架构索引](../architecture.md)

权威范围：把 M1 架构顺序转成可执行、可验收、可回滚的工程任务计划。本页不替代各主题页的架构事实；当任务计划与主题页冲突时，以主题页为准。

## 阶段目标

M1 在 M0 本机纵切基础上，交付三条可测试的最小能力线。其中 Pipeline 线和 Channel 线以**冻结协议对象、状态机和边界**为主要目标，Memory 线以**冻结存储格式、审批链路和低信任注入协议**为主要目标：

```text
1. Pipeline（冻结协议 + 产出 proposal artifact）
   pipe command 注册表 -> inspect / list / compose / dry-run
   -> typed pipeline AST -> JSONL event stream（含 step.failed）
   -> checkpoint -> PatchProposalArtifact -> 最小审批链路
   -> 通用 run、Pipeline Designer 编译优化、MCP 适配后置到 M2

2. Memory & Session Search（冻结格式 + 验证注入边界）
   session_search 索引触发 -> redacted manifest -> TTL/scope 隔离
   -> 手动 project note -> source_checking -> policy -> audit
   -> 低信任 data block 手动注入验证
   -> 自动 user/project memory、复习队列、遗忘曲线后置到 M2

3. Fake Channel Bridge（冻结 ingress/outbox 契约）
   fake provider + webhook verify -> ChannelEvent 归一化 -> binding 表
   -> ChannelAttachmentRef -> ChannelResourceGrant -> quarantine
   -> ChannelActionGrant -> outbox 队列（含并发 lease、query-before-retry）
   -> 真实 IM provider、多插件、远程附件下载优化后置到 M2
```

完成标准：

- 用户能在 Electron 中发现可用 pipe command（`pipe.list` + `pipe.inspect`），构造一条无副作用 typed pipeline，执行 `compose` 和 `dry-run`，看到 JSONL event stream（含 `step.failed`）和 checkpoint；dry-run 可在最后一步产出 `PatchProposalArtifact` 并进入最小审批链路。
- `session_search` 能索引已脱敏的会话摘要（用户手动触发 redaction），按 TTL 和 scope 隔离，查询返回 candidate ids 后由 daemon 二次授权生成 snippet。
- 用户能手动创建、编辑、搜索 project note；note 经过 `source_checking` 和 policy 校验后进入 memory store，不自动升级为 wiki claim。
- fake channel provider 能模拟入站 `channel.message.received`（含 webhook 验证、binding 表 actor 解析、role-based policy 拒绝）和出站 `ChannelActionGrant`（含 provenance），outbox 队列验证幂等、补偿、query-before-retry、并发 lease 和状态机。
- 所有新增副作用仍经过 policy、approval、WAL 和 audit；channel 事件不走捷径。

## 架构依据

| 依据 | 对任务计划的约束 |
| --- | --- |
| [MVP 顺序与主要风险](../architecture/roadmap-risks.md) | M1 收窄为 pipeline inspect/dry-run/compose、session_search + 手动 project note、fake/local channel provider。自动 memory、复习队列、真实 IM、Pipeline Designer 完整功能和通用 `run` 后置。 |
| [总览与核心分层](../architecture/overview.md) | 工程拆分围绕 `seaki-pipe`、`seaki-memory`、`seaki-channel` 和前端包展开；`seaki-pipeline`（Designer）完整功能后置，M1 只保留 pipe runtime 基础。 |
| [边界与权威链路](../architecture/boundaries.md) | 所有入口仍走 `daemon ingress -> inert event -> proposal/plan -> deterministic validation -> policy -> sandbox/broker -> audit/WAL/outbox`。新增 memory 写入只能走 `memory.propose`，不能热替换当前会话。 |
| [管道命令接口](../architecture/pipeline.md) | M1 pipeline 只实现无副作用命令；`side_effect_level=proposal_only` 的 patch artifact 必须由后续显式事务处理。`run` 等需要 policy/sandbox/checkpoint 契约稳定后开放。 |
| [记忆系统](../architecture/memory.md) | `session_search` 不保存原始 transcript，只存脱敏摘要、分片索引和引用指针，带 TTL/scope/删除机制。`user_memory`、自动 `project_memory`、复习调度后置。 |
| [Channel Bridge 插件化](../architecture/channel-bridge.md) | M1 不接真实 IM provider；fake provider 只验证 ChannelEvent 归一化、`ChannelActionGrant`、`ChannelResourceGrant`、outbox 幂等和 provenance。插件 secret 不泄露原则必须成立。 |

## 非目标

- 不接真实 Feishu / Slack / 企业微信 / Discord provider。
- 不实现自动 user memory、project memory 提议、复习队列调度或遗忘曲线完整实现。
- 不实现通用 `pipe run`、Pipeline Designer 完整编译优化、MCP 兼容适配层或 skills 调度。
- 不实现跨平台 sandbox 第二平台后端（M1 仍只保留 macOS Seatbelt）。
- 不做向量索引、实体图、远程 embedding 或跨 workspace memory 合并。
- 不让前端、插件或 agent 直接写入 memory store、channel outbox 或构造 `ChannelActionGrant`。

## 任务拆解

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
| --- | --- | --- | --- | --- |
| M1-00 | 工程骨架与 DTO 扩展 | `seaki-pipe` crate 骨架、`seaki-memory` crate 骨架、`seaki-channel` crate 骨架；DTO 扩展（`PipeCommandManifest`、`PipelineExecution`、`FrameEnvelope`、`PipelineError`、`ParagraphFrame`、`PatchProposalArtifact`、`MemoryItem`、`SessionSearchCandidate`、`ChannelEvent`、`ChannelActionGrant`、`ChannelResourceGrant`、`OutboxItem`）；TypeScript DTO 重新生成；M1 架构决策记录 | 新增 crate 能编译通过；DTO schema hash 校验通过；`cargo fmt --check`、`cargo clippy`、`cargo test` 通过；不提交生成物 | M0-11 |
| M1-01 | Pipe 命令注册表、inspect 与 list | `seaki-pipe` command registry、manifest 验证、`inspect` 与 `list` 实现、无副作用命令集（`wiki.search` 输出 `ParagraphFrame[]`、`citation.resolve`、`adr.summarize`、`filter`、`map`）和 `proposal_only` 命令（`wiki.patch.propose`） | `pipe.inspect("wiki.search")` 返回完整 JSON Schema、权限声明、副作用等级和资源限额；`pipe.list()` 返回所有已注册命令摘要，支持按 `side_effect_level` 过滤；`side_effect_level="proposal_only"` 的命令（如 `wiki.patch.propose`）在注册表中可被发现和 inspect；非法 command id 返回 `CommandNotFound`；schema hash 不匹配拒绝注册 | M1-00 |
| M1-02 | Pipeline compose 与类型检查 | `seaki-pipe` typed pipeline AST、`compose` 谓词约束、上下游类型匹配、cardinality 检查 | `compose` 拒绝类型不匹配或 cardinality 冲突（如 `many` 接到 `one` 下游）；能验证无副作用链条（`ParagraphFrame[] -> CitedParagraph[] -> TextAnswer`）和 `proposal_only` 链条（`CitedParagraph[] -> PatchProposalArtifact`）；失败策略（`fail_fast`、`skip`、`default`）在 AST 中声明但不执行 | M1-01 |
| M1-03 | Pipeline dry-run、错误协议与 proposal artifact | `seaki-pipe` dry-run 计划生成、权限预估摘要、JSONL event stream 输出（含 `step.failed`）、`PipelineError` 结构化错误、`PipelineExecution` 状态机、`FrameEnvelope` 包装；dry-run 产出 `PatchProposalArtifact`（当最后一步为 `proposal_only` 时）并接入 `wiki.patch.propose` 最小审批队列 | dry-run 输出预期读取范围、预期权限、预计 frame 数和 `PatchProposalArtifact`（当最后一步为 `proposal_only` 时），不产生实际副作用；JSONL stream 包含 `request`、`step.started`、`frame`、`checkpoint`、`step.completed`、`step.failed` 事件类型；`step.failed` 携带 `PipelineError`（含 `retryable`、`failed_step_id`、`error_kind`）；checkpoint 包含 input/output hash 和 frame offset；`PatchProposalArtifact` 通过 `wiki.patch.propose` 进入审批链路，复用 M0 已实现的 `WikiPatchTransaction` 和审批基础设施 | M1-02 |
| M1-04 | Session Search 索引触发、脱敏与查询 | `seaki-memory` session index（基于现有 `seaki-index` BM25 扩展）、redaction pipeline（最小正则 secret scan + 摘要提取）、redacted session manifest、TTL/scope 隔离、自动清理（启动扫描 + 写时惰性检查） | 用户可手动触发 redaction pipeline（M1 阶段 Electron 预览级 UI 无真实 session 结束信号；daemon 同时支持手动触发 API 供 M1-08 UI 调用）；摘要脱敏后进入 index，原始 transcript 不存入 memory store；TTL 过期条目先标记 `expired`，7 天后物理删除并生成 `AuditEvent`；查询先返回 candidate ids，daemon 按 actor/workspace/visibility 二次授权后生成 snippet | M1-00 |
| M1-05 | 手动 Project Note、搜索与 source_checking | `seaki-memory` project note 模型、note 标题+内容关键词 BM25 索引（复用 `seaki-index`）、note 搜索/查询、`memory.propose` -> policy -> audit 链路、note CRUD、与 wiki/source 边界校验、`source_checking` 最小实现（与现有 wiki claim 关键词/引用重叠检测） | 用户能创建/编辑/删除/搜索 project note；note 标题和内容关键词可被 BM25 索引和查询；note 提交后进入 `proposed -> scanning -> source_checking -> approved -> active`，与 [memory.md](../architecture/memory.md) 生命周期一致；`source_checking` 检测与 wiki claim 冲突，冲突则标记 `conflict` 并阻止进入 `approved`；note 内容冲突时以 wiki/source 为准，memory 降级为 stale；note 不可被 citation 直接引用，提升为权威知识必须走 wiki patch transaction；note 不自动升级为 claim | M1-00, M0-06 |
| M1-06a | Fake Channel Provider、Webhook 验证与 Actor 解析 | `seaki-channel` fake provider、FakeWebhookVerifier（签名、时间戳、防重放）、binding 表初始化（fixture/CRUD）、ChannelEvent 入站归一化、provider identity -> seaki_actor 解析 | fake provider 提交 `channel.message.received` 前必须经过 FakeWebhookVerifier（固定 secret + HMAC、seen_event_ids 防重放、明确失败码 `SIGNATURE_MISMATCH` / `TIMESTAMP_EXPIRED` / `EVENT_REPLAYED`）；binding 表支持 `provider_tenant_id + channel_binding_id + provider_user_id -> seaki_actor_id + workspace_role` 映射配置；Core 根据 binding 表解析 actor、role、channel_scope；`workspace_role=guest` 请求 `ChannelResourceGrant` 时 policy 拒绝并返回 `POLICY_DENIED_INSUFFICIENT_ROLE`；provider 不能声明 `seaki_actor_id` | M1-00 |
| M1-06b | Channel 附件授权与 Quarantine 模拟 | `seaki-channel` `ChannelAttachmentRef` 模型、`ChannelResourceGrant` 模型与 fake broker 下载到 quarantine 模拟（mock，不调用真实 `seaki-sandbox`） | 附件以 `ChannelAttachmentRef` 进入，经 `ChannelResourceGrant` 由 fake broker 下载到 quarantine 并生成 `observed_mime`、`content_hash`、`malware_scan_status`（mock）；`ChannelResourceGrant` 签发验证 scope、file_key、version；quarantine 为契约模拟，不调用真实 `seaki-sandbox` 的 `source-ingest` profile；附件进入 wiki 的跨线端到端验证后置 M2 | M1-06a |
| M1-07 | Channel Action Grant、出站 Outbox 与并发验证 | `seaki-channel` `ChannelActionGrant` 签发/消费（含 `uses_remaining`）、`OutboxItem` / `ChannelSendAttempt` 模型、FakeProviderQueryAPI、幂等 key、补偿动作、并发 lease 抢占测试 | `ChannelActionGrant` 包含 scope、audience、ttl、`uses_remaining`、idempotency key、允许动作类型和 provenance（`transaction_id`、`source_id`、citation ids、thread scope、audit id）；`uses_remaining` 递减到 0 后再次使用失败；outbox 状态机：`pending -> leased -> sending -> sent / failed -> retry / compensated`；`compensated` 在 send 确认失败且不可重试时触发；同一 idempotency key 不能重复发送；`unknown` 状态必须先调用 FakeProviderQueryAPI（按 `provider_idempotency_key` 查询 `sent` / `not_found` / `failed`）再 retry；并发测试验证：多个 dispatcher worker 同时 lease 同一 `pending` item，仅一人成功 | M1-06b |
| M1-08 | 前端扩展：Pipeline / Memory / Channel UI | Electron 新增 PipelineDryRun、SessionSearch、ProjectNoteEditor、ChannelStatus / OutboxViewer screens；DTO、state、domain use case 同步；SessionSearch UI 支持手动触发 redaction 和"引用到当前会话"操作；ProjectNoteEditor 支持 note 搜索 | UI 能把用户意图转成 `pipeline.dryRun`、`memory.propose`、`channel.outbox.query` domain use case；pipeline dry-run 结果以 JSONL 或结构化摘要展示，含 `PatchProposalArtifact` 预览和进入审批入口；SessionSearch 中用户可手动触发 redaction pipeline 和将 candidate 以低信任 data block（`taint=untrusted_content`）引用到当前 agent context，**不进入 system prompt**；ProjectNoteEditor 支持 note 的搜索、创建、编辑和删除；channel outbox 状态可见 | M1-03, M1-05, M1-07 |
| M1-09 | 端到端验收与发布门禁 | M1 demo fixture、pipeline dry-run + proposal artifact e2e smoke test、session search + project note 搜索与 source_checking e2e、fake channel 入站/webhook/outbox e2e、M1 操作手册 | `pipe.list` -> `pipe.inspect` -> `compose` -> `dry-run` -> 产出 `PatchProposalArtifact` -> 进入审批入口 的 happy path 可重复；session search 查询到 candidate 并二次授权；project note 创建-搜索-`source_checking`-审批-查询-删除闭环；低信任 data block 注入边界通过前端状态测试 + daemon 单元测试验证（transport mock 阶段不承诺完整 e2e）；fake channel webhook verify -> 入站事件 -> `ChannelResourceGrant` -> outbox -> `unknown` query-before-retry -> 状态验证可重复；并发 lease 抢占通过集成测试；所有质量门禁通过 | M1-08 |

## 推荐执行顺序

1. 先完成 M1-00，锁定工程布局、DTO 扩展和新 crate 编译基线。
2. 再并行推进 M1-01 ~ M1-03（Pipeline 线）和 M1-04 ~ M1-05（Memory 线）和 M1-06a ~ M1-07（Channel 线）。三条线内部串行，线之间尽量解耦，可并行。
3. 然后完成 M1-08，把三条线的能力集成到 Electron UI。
4. 最后完成 M1-09，固化端到端验收和操作手册。

每个任务都按“编码 -> 测试 -> 审阅 -> 修复 -> 提交”闭环推进，并在任务开始时声明至少一种验证类型：unit、integration、UI replay 或 e2e smoke。如果实现过程中发现架构事实需要调整，先更新对应主题页，再更新本计划和维护记录。

## 质量门禁

最小门禁：

- Rust：`cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。
- TypeScript/Electron：`pnpm typecheck`、`pnpm lint`、`pnpm test`。
- 文档：新增或修改 Markdown 后检查相对链接。
- Git：不得提交 `dist/`、`target/`、`.omx/`、本机系统文件或依赖目录。

关键回归测试：

- pipe command 注册拒绝 schema hash 不匹配或缺少必填 manifest 字段；`pipe.list` 枚举完整且支持按 `side_effect_level` 过滤。
- `compose` 拒绝类型不匹配、cardinality 冲突和循环依赖；能验证无副作用链条（`ParagraphFrame[] -> CitedParagraph[] -> TextAnswer`）和 `proposal_only` 链条（`CitedParagraph[] -> PatchProposalArtifact`）。
- dry-run 不产生实际副作用；dry-run 输出包含预期权限、预期读取范围和 `PatchProposalArtifact`（当适用时）；`step.failed` 携带结构化 `PipelineError`。
- session_search 索引不保存原始 transcript；redaction pipeline 在会话结束时自动触发；TTL 过期条目先标记 `expired`，7 天后物理删除并生成 `AuditEvent`。
- project note `memory.propose` 生命周期必须包含 `source_checking` 阶段，与 [memory.md](../architecture/memory.md) 一致；冲突时以 wiki/source 为准，memory 降级 stale；note 不可被 citation 直接引用；note 标题和内容关键词可被 BM25 搜索。
- `memory.propose` 不热替换当前会话系统提示；memory 注入只能作为低信任 data block；SessionSearch UI 的"引用到当前会话"操作验证此边界。
- fake provider 不能声明 `seaki_actor_id`、workspace role 或 policy decision；webhook 签名/时间戳/防重放验证失败返回明确错误码；`workspace_role=guest` 请求 `ChannelResourceGrant` 时 policy 拒绝。
- `ChannelActionGrant` 过期或 `uses_remaining` 用尽后再次使用失败。
- outbox 同一 idempotency key 不能重复发送；`unknown` 状态必须先调用 FakeProviderQueryAPI 查询结果再 retry。
- channel 附件只以 `ChannelAttachmentRef` 进入；`ChannelResourceGrant` 签发/消费验证 scope、file_key、version 和 `uses_remaining`；下载到 quarantine 后生成 `observed_mime`、`content_hash`、`malware_scan_status`（mock，不调用真实 sandbox）；`ChannelActionGrant` payload 携带 provenance（`transaction_id`、`source_id`、citation ids、thread scope、audit id）。
- 并发场景：多个 dispatcher worker 同时 lease 同一 `pending` outbox item，仅一人成功；多用户并发入站事件，Core 正确解析不同 actor 和 role。

## 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| Pipeline 过早开放 `run` 导致副作用失控 | M1 明确只实现 `inspect/compose/dry-run`；`run` 等 policy/sandbox/checkpoint 契约稳定后开放。 |
| Memory 污染 wiki 权威事实 | `memory.propose` 必须经过 policy 和 audit；与 wiki/source 冲突时 memory 降级 stale；memory 不替代 wiki claim。 |
| Session search 泄露完整会话 | 原始 transcript 不进入 memory store；只存脱敏摘要和引用指针；带 TTL 和 scope 隔离。 |
| Channel 事件绕过权威链路 | 所有 ChannelEvent 必须经过 ingress -> normalize -> inert event -> proposal -> policy；插件不能声明 actor 或 policy decision。 |
| Fake provider 验证不足导致 M2 接入真实 IM 时返工 | M1 fake provider 验证 ChannelEvent 归一化、webhook verify、`ChannelActionGrant`、`ChannelResourceGrant`、outbox 幂等、provenance 和并发 lease；M2 替换 provider 实现层**并补全**真实网络延迟、多租户 scale、IM 平台特有错误码，以及 Channel 附件从 quarantine 到 `source.ingest` 的真实 sandbox 链路。 |
| 前端绕过 daemon 直接操作 memory / channel | 前端只调用 domain use case；memory write 和 channel outbox 都回到 daemon。 |

## 交付物清单

- `seaki-pipe`：command registry、manifest、`inspect`、`list`、`compose`、dry-run、JSONL stream（含 `step.failed`）、`FrameEnvelope`、`PipelineError`、`ParagraphFrame`、`PatchProposalArtifact`。
- `seaki-memory`：session search index（含 redaction pipeline 和自动触发机制）、project note（含 `source_checking`）、memory.propose 链路、TTL/scope 隔离与审计清理、低信任 data block 注入验证。
- `seaki-channel`：fake provider、FakeWebhookVerifier、binding 表初始化、ChannelEvent 归一化、`ChannelAttachmentRef`、`ChannelResourceGrant`、fake broker quarantine 下载（mock）、`ChannelActionGrant`（含 `uses_remaining` 和 provenance）、`OutboxItem`、FakeProviderQueryAPI、幂等验证、并发 lease 抢占。
- Electron 新增 screens：PipelineDryRun、SessionSearch、ProjectNoteEditor、ChannelStatus、OutboxViewer。
- 可重复 demo fixture 和端到端 smoke test（pipeline proposal artifact、memory source_checking + 低信任注入、channel webhook + outbox query-before-retry + 并发 lease 各一条）。
- 与实现同步的架构文档更新、M1 操作手册和已知风险列表。

## 暂缓到后续阶段

- M2：
  - Pipeline Designer 完整编译优化、`pipe run`、MCP/skills 适配层。
  - 自动 user memory、project memory 提议、遗忘曲线完整实现、复习调度。
  - 真实 Channel Bridge、多 IM 插件、远程附件导入。
  - 跨工具 connector、`RunbookIndex`、自动 topic clustering 和 review-learning。
