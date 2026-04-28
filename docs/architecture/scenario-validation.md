# 虚拟需求推演收敛

[返回架构索引](../architecture.md)

权威范围：飞书 PDF、决策考古、个人知识复利、跨工具孤岛、本机导入问答和规格收敛。

## 虚拟需求推演收敛

多条虚拟需求推演给出的共同结论：架构方向可行，但首版必须用更窄的纵切验证，不应一开始同时追求完整 pipeline、完整 memory、完整插件生态和多平台 sandbox。seaki 的高价值场景不是一次性工具调用，而是把“当时为什么这样做”“资料和洞察如何复用”“跨工具知识如何统一入口”沉淀为可引用、可审计、可续写的本地知识层。

### 飞书 PDF 纳入 wiki

安全闭环应拆成确定性 DAG：

```text
feishu.webhook.verify
-> channel.event.normalize(message + attachment refs)
-> core.create_inert_event
-> policy.preflight(channel.resource.read + source.ingest)
-> secret_broker.issue_scoped_drive_download_grant
-> channel_broker.download_attachment_to_quarantine
-> source.add CAS raw/source manifest
-> pdf.extract raw blob in sandbox
-> parsed frames with security flags
-> agent proposes summary + wiki patch with citations
-> deterministic validation + policy
-> policy.check(wiki.patch + channel.reply)
-> WAL commit source/wiki/audit + outbox pending item
-> outbox dispatcher issues ChannelActionGrant
-> provider send with idempotency key
-> audit finalize
```

缺口已收敛为规格要求：飞书身份映射、`ChannelAttachmentRef`、`ChannelResourceGrant`、PDF sandbox profile、wiki patch 审批门槛、WAL/outbox 绑定和 IM answer provenance。

其中 outbox item 必须与 source/wiki/audit 在同一 WAL 或事务批次创建；实际发送由 dispatcher 异步推进 `pending -> sending -> sent|unknown|failed|compensated`。如果 provider send 超时，必须先按 provider client token 查询发送结果，再决定是否重试，不能盲目发送第二条消息。

### 决策考古

场景：产品团队半年前决定不做某功能，新负责人追问“为什么当初这样定”。如果只依赖人的记忆、会议纪要文件名和共享盘路径，团队会重新调研同一个问题。

seaki 的闭环应是：

```text
meeting notes + compliance mail screenshot + product discussion
-> source.add CAS
-> parsed frames with provenance
-> decision-log.extract
-> ADR patch proposal
-> citation validation
-> approval diff
-> wiki/decision-log.md commit
-> search by feature/compliance/owner/date
```

wiki 嵌入方式：建立 `Decision Log` 页面，每条决策使用 ADR 格式：背景、选项、否决理由、决策人、日期、证据引用和复审条件。合规邮件截图、会议纪要和讨论记录都作为 source，不把截图内容改写成无来源结论。

发现的问题：

- ADR 不是普通 wiki 段落，必须是一等对象或受 schema 约束的页面块，字段包括 `decision_id`、`status`、`decided_at`、`owners`、`options`、`rejected_reasons`、`review_after`、`citations`。
- “为什么不做”必须保留 rejected alternatives，否则半年后只剩结论，无法解释权衡。
- 邮件截图、IM 截图和会议纪要都可能含敏感信息，source preview 和 citation resolve 必须走权限校验。
- 决策被推翻时不能覆盖旧 ADR；应追加 supersede 关系，保留考古链路。

结果目标：30 秒定位原始证据，避免重复论证；新成员理解“系统为什么长这样”，不是凭直觉推翻。

交付分期：M0 只承诺人工确认的 ADR block、evidence picker、citation validation 和搜索回跳；`decision-log.extract` 只能生成 proposal。自动从截图/IM 中抽取决策、复审提醒和跨工具证据同步放到 M1/M2。

### 个人知识复利

场景：独立顾问或创作者每年研究多个行业，几年后再次遇到同一主题。传统文件夹、云盘和全文搜索只能找文件，不能把论文批注、访谈笔记、数据表和报告洞察聚合成一个持续演进的概念页。

seaki 的闭环应是：

```text
old laptop / cloud drive / PDFs / notes / datasets
-> user-selected file grants
-> source.add CAS
-> topic clustering proposal
-> concept page patch
-> embedded source cards + annotations
-> follow-up writing on same concept page
-> answer/report draft with citations
```

wiki 嵌入方式：按概念组织第二大脑，例如 `跨境支付` 页面，而不是按文件格式或年份分类。页面内保留 PDF、外部数据库链接、访谈摘要、批注、当时判断和后续修正。

发现的问题：

- topic clustering 只能生成 proposal，不能自动重排用户知识库；概念页创建、合并、重命名都需要 review。
- 旧电脑路径、云盘链接、外部数据库链接会失效，因此 citation 需要区分 `local_cas_source`、`external_link`、`imported_snapshot` 和 `missing_source`。
- 同一主题跨年份续写时，需要 `claim supersede` 和 `temporal context`，否则 2024 年判断会污染 2026 年回答。
- memory 只能保存工作偏好和复用线索，不能替代 wiki/source 事实；真正可引用内容仍来自 source、claim 和 citation。

结果目标：把 6 小时资料重找压缩到 45 分钟以内；零散研究随着概念页积累产生复利，而不是每次从零开始。

交付分期：M0 只承诺手动创建/合并 `ConceptPage`、source card、annotation 和 citation 类型区分；`topic clustering proposal` 只作为待审建议。跨年份自动聚类、外部数据库同步和大规模主题图谱放到 M1/M2。

### 跨工具孤岛

场景：10 人技术团队的文档散落在 Notion、GitHub Wiki、飞书文档和个人语雀。on-call 工程师半夜处理报警，需要在 10 分钟内找到报警含义、关联代码、回滚命令和历史事故。

seaki 的闭环应是：

```text
alert received
-> search incident/runbook wiki
-> resolve structured links
-> fetch allowed snippets from Notion/GitHub/Feishu/Yuque adapters
-> rank by service + alert name + freshness
-> show operation room page
-> execute only approved local/runbook actions
-> append incident notes as proposal
```

wiki 嵌入方式：建立 `运维作战室`，不迁移原有文档，而是作为索引层和叙事层。每个故障类型对应一个 wiki 页面，页面结构化链接到需求背景、代码位置、部署/回滚手册和历史事故复盘。

发现的问题：

- 跨工具 connector 必须是 source adapter，不是隐式同步器；远程内容进入回答前必须有 source snapshot、fetch time、权限 scope 和 citation。
- on-call 场景不能依赖 LLM 临场多轮找工具，必须预先有 `service -> alert -> runbook -> rollback` 的 typed relation。
- 回滚命令属于高风险副作用，只能展示或生成 approval request，不能由 wiki 页面文本直接执行。
- 外部工具权限不一致时，搜索结果必须按当前 actor 过滤；不能因为 wiki 页面索引到了链接就绕过 Notion/飞书/GitHub 权限。

结果目标：wiki 成为跨工具唯一入口，MTTR 从几十分钟压缩到十几分钟；工程师不需要记住知识在哪个 App 里，只需要从故障页进入。

交付分期：M0 不接真实跨工具 fetch 和回滚执行；M1 先做预登记 `RunbookIndex`、只读链接、手动 citation 和 incident note proposal；M2 再接 Notion/GitHub/飞书/语雀 adapter、权限同步和 runbook approval flow。

### 本机导入到问答

首版最小用户闭环：

```text
workspace.init
-> file capability grant
-> source.add CAS
-> markdown/pdf parsed frames
-> wiki.home.generate patch
-> approval diff
-> WAL commit
-> BM25 index rebuild
-> Electron search/read
-> citation/source visibility revalidation by actor + thread scope
-> optional IM text answer with citations
```

必须明确的数据生命周期：

- raw blob 是权威，parsed frames 是可重建派生物。
- wiki patch schema 必须有 base revision、citation validation 和 rollback marker。
- index rebuild 失败只标记 stale，不破坏已提交事务。
- IM answer payload 必须带 citation ids、source ranges、thread scope 和 audit id。
- 决策日志、概念页和运维作战室都可以从这条纵切演进出来：先解决本机 source、wiki patch、citation 和搜索，再接入跨工具 connector。

### 重要模块推演矩阵

本轮按重要模块分别推演虚拟需求，统一结论是：seaki 的方向成立，但必须先把协议对象、状态机和幂等边界冻结，再扩展到多 IM、多端和通用 pipeline。

| 模块 | 虚拟需求 | 可行性结论 | 发现的问题 |
| --- | --- | --- | --- |
| Core / Policy / Sandbox | Electron 选择本机 PDF，导入 wiki 并生成可引用摘要 | 可行，但必须保持唯一权威链路 | `source.ingest` 状态机、opaque capability grant、PDF resource limit、parsed frame 安全标记和 WAL A/B 边界仍需规格化 |
| Pipeline / PCI | 统计所有 wiki 页中提到 Rust sandbox 的段落，按来源可信度排序，生成待审 patch | 可行，但首版只能是 `proposal_only + dry-run` typed pipeline | 缺 `ParagraphFrame`、trust ranking contract、stream checkpoint/error protocol、patch proposal artifact schema 和 MCP adapter attestation |
| Wiki / Source / Index | 3 个 Markdown + 2 个 PDF 导入、搜索、删除其中一个 PDF 后继续问答 | 可行，但 citation / tombstone / stale index 是主要风险 | 需要 `SourceManifest`、`ParsedFrame`、`Claim`、`Citation`、`IndexGeneration` 等一等对象；查询必须回查 source/citation 权威状态 |
| Channel Bridge | 飞书群多人并发上传文件，权限不同、webhook 重放、回复重试 | 可行，但风险中高 | tenant / workspace / actor 不能混用；附件必须绑定 message/file/version；outbox 需要 provider-side idempotency 和 `unknown -> query-before-retry` 状态 |
| Memory / Review | 记录项目约定、研究偏好和复用线索 | 可行，但 memory 不能成为第二事实源 | memory 只能保存辅助线索和用户偏好；ADR、概念页、运维知识必须以 source/wiki claim 为权威 |
| Frontend / SDK | 首启、导入、审批、搜索、IM citation 回跳 | 可行，但仅限 Electron MVP 纵切 | `@seaki/dto/domain/state` 需要 DTO、事件 envelope、任务 replay、错误 recoverability 和 daemon 权威 citation resolver |

### 汇总审核结论

必须冻结的 P1 规格：

- 唯一执行链路固定为 `ingress authenticate/normalize -> Core inert event -> agent proposal -> deterministic compiler -> policy -> sandbox/broker -> WAL/audit/outbox`。
- `source.ingest` 必须有状态机：`selected -> grant_requested -> granted -> raw_committed -> parse_running -> parsed|partial|failed -> patch_proposed -> approval_pending -> committed|denied -> indexed|index_stale`。
- capability 必须是 opaque grant，内部绑定 `workspace_id`、`actor`、`audience`、`operation`、`max_bytes`、`mime_constraints`、`canonical_path_hash`、`idempotency_key`、`jti`、`uses_remaining`、`expires_at`、revocation 和 `policy_decision_id`。
- Channel 入站只能提交 provider 身份；`seaki_actor_id`、`workspace_role`、workspace binding 和 channel scope 必须由 Core 解析。
- 所有外部内容默认 untrusted；taint 必须随 parsed frame、index candidate、memory candidate、agent context 和 patch proposal 传播，不能升级为权限、命令、system prompt 或审批结论。
- raw source、wiki claim、citation registry 是回答权威；index、memory、parsed frames 是可重建或可撤销的辅助层，不能直接越权成为事实源。
- PCI 只产出 `PatchProposalArtifact`、dry-run plan 和 typed frames；真正 apply 只能由 `WikiPatchTransaction` 执行。

必须补齐的 P2 对象模型：

- `Task` / `Transaction` / `AuditEvent` / `ApprovalRequest` / `CapabilityGrant`：核心执行、审批、WAL 和审计契约。
- `SourceManifest`：source 生命周期、权限、tombstone、parse status。
- `ParsedArtifact` / `ParsedFrame` / `ParagraphFrame`：parser run、版本、range kind、text hash、trust level、security flags。
- `Claim` / `Citation`：claim 到 source/frame/range 的关系、validation status、visibility。
- `WikiPatchProposal`：base revision、diff、claim ids、citation validation、risk summary、rollback marker。
- `IndexGeneration`：index schema version、覆盖的 wiki/source revision、fresh/stale/failed 状态。
- `PipelineExecution` / `PipelineStepRun` / `FrameEnvelope` / `Checkpoint` / `PipelineError`：pipeline 运行、streaming、checkpoint 和恢复契约。
- `ChannelAttachmentRef` / `ChannelResourceGrant` / `ChannelActionGrant` / `OutboxItem` / `ChannelSendAttempt`：附件身份、下载授权、出站授权、provider 幂等与补偿状态。
- `DecisionRecord` / `ConceptPage` / `RunbookIndex`：`seaki-wiki/schema` typed page block，包含 ADR 字段、主题聚合、跨工具链接、复审状态和证据引用。
- `Annotation` / `SourceCard`：source range、用户备注、生成摘要、supersede/conflict、visibility 和 citation refs。
- `MemoryItem`：scope、provenance、trust level、confirmed_by、expires_at、confidence、stale/conflict 状态。
- `FrontendEventEnvelope`：`event_id`、`schema_version`、`payload_schema_hash`、`seq`、`actor_id`、`scope`、`workspace_id`、`task_id`、`transaction_id`、`correlation_id`、`causation_id`、`revision`、`occurred_at`、`replayable`、`idempotency_key`。

首版产品边界必须继续收窄：

- M0 只做 Electron + Rust daemon + 本机文件导入 + typed wiki page/claim + BM25 candidate search + approval diff + citation-backed answer + citation 回跳。
- Channel Bridge、真实 IM、附件导入、Pipeline Designer、自动 memory、跨工具 connector 放到 M1/M2；M0 只冻结对象模型和安全准入门槛。
- 决策、概念和运维事实必须进入 wiki/source/citation，不自动写入长期 memory。

Dogfood 验收指标：

- 从本机 source 导入到 citation-backed answer 的成功率。
- citation 回跳到 source range 或 wiki anchor 的成功率。
- 30 秒内定位一个已记录决策证据的成功率。
- approval diff 中 claim 级 citation validation 覆盖率。
- stale / denied / degraded citation 的正确降级率。
