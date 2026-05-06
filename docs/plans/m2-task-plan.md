# M2 阶段任务计划：Pipeline / Agent / Channel / Memory 纵切

> 本计划按用户选择的「Pipeline + Agent 优先」顺序安排 M2 任务。详细架构事实以 `docs/architecture/` 各主题页为准；任务计划与主题页冲突时，以主题页为准。

## 阶段目标

M2 在 M0（本机纵切）和 M1（Pipeline/Memory/Channel 骨架）基础上，交付完整的 AI 工具组合、Agent 运行时、真实 IM 接入和智能记忆系统：

```text
User Intent / IM Message
  -> Pipeline Designer（意图编译 + 类型检查 + 权限预估）
  -> Agent Runtime（skills 调度 + LLM 调用 + MCP 适配）
  -> Pipe Runtime（真实执行 + checkpoint + approval gate）
  -> Sandbox / Broker / Channel
  -> Citation-backed answer + Outbox reply
```

完成标准：
- Pipeline Designer 能将用户意图编译为带类型检查、权限预估和成本估算的 pipeline graph。
- Agent Runtime 能调度 skills、调用 LLM、管理 session compaction，并通过 MCP 层与外部工具互操作。
- Pipeline 能在 sandbox 中真实执行（非 dry-run），支持 tee/branch/join DAG、streaming checkpoint 和 per-step approval。
- 至少一个真实 IM 插件（飞书）接入，支持消息收发、附件导入和 provenance 回写。
- Memory 系统从手动 note 升级到自动 user/project memory 收集、遗忘曲线调度和 review-learning。
- 所有关键路径有 E2E 验收，拒绝路径有回归测试。

## 架构依据

| 依据 | 对任务计划的约束 |
|------|----------------|
| [MVP 顺序与主要风险](../../architecture/roadmap-risks.md) | M2 交付 Pipeline Designer、MCP 适配、真实 Channel Bridge、自动 memory、review-learning；跨平台 sandbox 和完整多端仍后置。 |
| [总览与核心分层](../../architecture/overview.md) | 新增 `seaki-pipeline` 和 `seaki-agent` crate；`seaki-pipe` 从 dry-run 模拟器升级为可执行运行时。 |
| [边界与权威链路](../../architecture/boundaries.md) | Pipeline 和 Agent 不能绕过 policy；所有 side-effect 步骤必须经过 approval gate；Channel 事件必须走 ingress -> inert event -> proposal 链路。 |
| [管道命令接口](../../architecture/pipeline.md) | Pipeline Designer 输出 typed AST；文本 shell-pipe 仅作展示；每步有 CPU/内存/帧数/帧大小/超时限制；downstream 不能继承未声明的权限。 |
| [Rust Sandbox Runtime](../../architecture/sandbox-runtime.md) | Pipeline 执行步骤的 sandbox profile 与 policy 映射到现有 capability 体系；不新增平台后端。 |
| [Channel Bridge 插件化](../../architecture/channel-bridge.md) | 插件只做协议适配，不持有 secret、不直接读写文件/模型/命令；附件必须经 quarantine 和 malware scan 后才能 source.add。 |
| [记忆系统](../../architecture/memory.md) | Memory 不是 wiki 替代，不能覆盖 source/wiki 权威；frozen snapshot 在 session 开始时生成；mid-session 写入不热替换当前上下文。 |

## 非目标

- 不实现 Pipeline 的分布式执行或跨机器调度。
- 不实现 Agent 的自主长期运行（long-running autonomous loop）；Agent 由用户触发或 IM 事件触发。
- 不实现跨平台 sandbox 新后端；继续使用现有 macOS Seatbelt 抽象。
- 不实现完整多端（Web/RN/小程序/鸿蒙）；M2 前端仍聚焦 Electron。
- 不做通用自然语言到任意工具调用的零样本映射；skill 和 command manifest 必须预先注册。
- 不实现 memory evolution 的自动上线；演化门禁基础设施就位，但实际优化需人工审批后生效。

---

## 任务拆解

### 阶段一：Pipeline 核心引擎（M2-P01 ~ M2-P05）

> 前置条件：`seaki-pipe` 已具备 AST、Registry、6 个内置命令、dry-run 模拟。M2 在此基础上补齐 Pipeline Designer 编译器和真实执行运行时。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M2-P01 | Pipeline Designer — 意图编译与图构建 | `seaki-pipeline` crate 骨架；`PipelineGraph` DSL / typed AST；意图解析器（LLM → graph）；Command Manifest 注册与版本管理 | 给定自然语言意图，能生成带 `cmd`/`args`/`in`/`out` schema 的 pipeline graph；未知命令在编译期报错；schema hash 不匹配拒绝连接上下游 | M1 已交付骨架 |
| M2-P02 | Pipeline Designer — 类型检查、权限预估与成本估算 | 类型检查器（FrameType/Cardinality 跨步骤验证）；Policy Estimation（整体 pipeline 所需 capability 汇总）；Token/Cost 估算器 | 类型不匹配、cardinality 冲突在编译期拒绝；权限超出当前 actor scope 的 pipeline 在编译期拒绝并给出缺失 capability 清单；cost 估算误差不超过 2 倍 | M2-P01 |
| M2-P03 | Pipe Runtime — 从 dry-run 到真实执行 | Pipe Runtime（执行引擎）；streaming frame processing；per-step policy check；step 级 resource limit 强制（CPU/内存/帧数/帧大小/超时）；`PipelineStepRun` 结构化错误 | dry-run 通过的 pipeline 能按步骤真实执行；每步执行前有 policy check；每步有 audit 记录；资源超限触发 `PipelineError::ResourceExceeded` 并终止 | M1 骨架, M2-P02 |
| M2-P04 | Pipe Runtime — DAG 控制流与容错 | `tee`/`branch`/`join` 算子；checkpoint 与 resume；局部 retry 边界；compensating action 钩子 | 复杂 DAG 能正确执行；中断后能从最近 checkpoint 恢复；失败步骤在 retry 边界内重试；compensating action 在 rollback 时调用 | M2-P03 |
| M2-P05 | Pipeline — Approval Gate 与事件流 | Approval Gate 集成（整个 pipeline 在需要审批的步骤前暂停）；JSONL 事件流（`step.started`/`frame`/`checkpoint`/`step.completed`）；pipeline 状态机（`pending`/`running`/`awaiting_approval`/`completed`/`failed`/`cancelled`） | 需要审批的步骤触发 `ApprovalRequest`，pipeline 进入 `awaiting_approval`；审批通过恢复执行，拒绝进入 `failed` 并触发 compensating action；事件流可被前端订阅回放 | M0-07 (Approval diff), M2-P04 |

### 阶段二：Agent Runtime 与 MCP 适配（M2-A01 ~ M2-A04）

> 前置条件：无；`seaki-agent` 为新建 crate。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M2-A01 | Agent Runtime 骨架 — 模型调用与 Session 管理 | `seaki-agent` crate；LLM API 抽象（支持多 provider）；Session State Machine（`idle`/`planning`/`executing`/`awaiting_user`/`compacting`）；Session Compaction（历史摘要 + 关键 claim 保留） | 能发起模型调用并返回结构化输出；session 状态正确流转；compaction 后 session 长度缩减且关键决策点不丢失； compaction 产物写入 WAL | M2-P05 |
| M2-A02 | Skills 调度器 | Skill Registry（skill manifest + 准入条件 + 所需 capability）；Skill 匹配与调度（用户 intent → skill → pipeline template）；Skill 上下文注入（注入 memory snapshot、wiki claims、session history） | 用户请求能路由到匹配的 skill；skill 能声明所需 memory/wiki/source scope 并在执行前验证；未满足准入条件的 skill 不被调度 | M2-A01 |
| M2-A03 | MCP 兼容层 — mcp-to-pipe | MCP Server Discovery；MCP Tool → Pipe Command 包装器（`mcp-to-pipe` adapter）；MCP resource 映射到 seaki capability grant | 外部 MCP tool 能在 pipeline 中作为步骤调用；MCP tool 的 schema 自动转换为 command manifest；调用前通过 policy check | M2-P03 |
| M2-A04 | MCP 兼容层 — pipe-to-mcp 与 Agent 集成 | Pipe Command → MCP Resource 暴露器（`pipe-to-mcp` adapter）；Agent 驱动 Pipeline Designer 生成并执行 pipeline 的闭环 | seaki 的 pipe command 能被外部 MCP 客户端调用；Agent 接收到用户 intent 后能生成 pipeline、触发 dry-run、请求 approval、执行并返回 citation-backed answer | M2-A02, M2-A03 |

### 阶段三：Channel Bridge 真实化（M2-C01 ~ M2-C05）

> 前置条件：`seaki-channel` 已具备 Outbox 模型、fake provider、webhook 校验、资源授权模型。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M2-C01 | Channel Bridge 运行时 — WASM 插件与 Secret Broker | WASM 插件运行时（WASMtime 或类似）；插件目录结构（`plugins/channel/<name>/`）；`plugin.toml` manifest 解析；Secret Broker（scoped opaque token 解析，插件不可见 bearer token） | 插件能加载并声明 capabilities；secret broker 能按 scope 解析 token 但不返回给插件；插件尝试读取 secret 被拒绝并审计 | M1 骨架 |
| M2-C02 | Channel Bridge — Ingress 与身份映射 | Ingress auth/normalization（signature verification、timestamp check、event ID dedup）；Identity Mapping（`provider_tenant_id` + `channel_binding_id` + `provider_user_id` → `seaki_actor_id`、`workspace_role`）；`ChannelEvent` 归一化 | 伪造签名、过期时间戳、重复 event ID 被拒绝；IM 用户正确映射到 seaki actor 和 workspace role；未映射用户进入 guest 或拒绝策略可配置 | M2-C01 |
| M2-C03 | 远程附件导入 — Quarantine 与资源授权 | `ChannelResourceGrant` 发放与校验；附件下载隔离区（quarantine）；observed mime/size/hash 计算；malware scan stub（至少校验 hash 和 mime 一致性） | 附件下载到隔离区，完成元数据校验后才能进入 `source.add`；mime 不一致、hash 失败或扫描不通过进入 `failed` 并审计；插件不能直接访问隔离区文件 | M2-C02 |
| M2-C04 | Outbox 调度器与真实 Provider 驱动 | Outbox Dispatcher（lease-based 领取、重试、退避、compensating action）；Provider Idempotency Key 调和（处理 `unknown` 状态）；Fake Provider 替换为真实 Provider 驱动器接口 | outbox item 能可靠投递；provider 返回 `unknown` 时通过幂等查询调和状态； lease 过期能被其他 dispatcher 实例安全接管 | M1 骨架, M2-C01 |
| M2-C05 | 飞书插件实现 | 飞书 protocol adapter（消息收发、附件、thread reply、群聊/单聊）；飞书事件签名验证；飞书文件下载对接 Core-owned HTTP client | 飞书消息能进入 seaki 并生成 `ChannelEvent`；seaki 回复能回到飞书正确 thread；附件经 quarantine 后进入 source ingest；provenance 回写包含 transaction_id 和 citation | M2-C02, M2-C03, M2-C04 |

### 阶段四：Memory 智能化与 Review Learning（M2-M01 ~ M2-M04）

> 前置条件：`seaki-memory` 已具备 `NoteStore`（手动 project note）、`SessionSearchIndex`（手动脱敏 BM25）、TTL 清理。

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M2-M01 | 自动 Memory — user/project memory 收集 | 自动 memory 收集器（从 session history、wiki patch、approval decision 中提取）；`MemoryItem` 生命周期状态机（`proposed → scanning → source_checking → approved\|rejected → active → stale\|conflict\|expired → archived\|deleted`）；Conflict Detection（memory 与 wiki/source 冲突时降级为 hint） | 关键用户偏好、项目约定能从会话中自动提取为 `MemoryItem`；冲突时 memory 降级为 `stale`/`hint`，不覆盖 wiki/source 权威；未确认的 memory 不能作为自动执行依据 | M1 骨架 |
| M2-M02 | Memory — Frozen Snapshot 与写入管道 | Frozen Snapshot（session 启动时生成 memory snapshot，注入 context）；`memory.propose` 管道（policy check → injection scan → duplicate detection → scope binding → audit）；Mid-session 写入进入 WAL 但不热替换当前上下文 | snapshot 在 session 开始时固定；mid-session memory 写入能被后续 session 读取但当前 session 上下文不变；injection scan 拒绝包含指令注入的 memory proposal | M2-M01 |
| M2-M03 | Review Learning — 遗忘曲线与复习调度 | 遗忘曲线调度器（`retention(t) = exp(-elapsed_days / stability_days)`）；Review Queue（按 `next_review_at` 排序）；Grading Feedback（`again`/`hard`/`good`/`easy` → 调整 `stability_days`）；Review CLI / API | card 能按遗忘曲线正确调度；grading 后 stability_days 合理调整（again 显著缩短，easy 显著延长）；到期的 review items 能被查询和作答 | M2-M02 |
| M2-M04 | Review Learning — 卡片生成与 Topic Clustering | `review-learning` skill（从 wiki/source/notes/session summaries 生成 review cards）；自动 Topic Clustering（基于 claim 和 citation 的 topic 聚类）；`RunbookIndex`（按 topic 组织的可执行操作手册索引） | wiki/source/notes 能自动生成 review cards 并进入调度队列；topics 能自动聚类且聚类结果可人工校正；RunbookIndex 能按 topic 检索到相关 pipeline template 和 runbook | M2-M03 |

### 阶段五：前端配套与 E2E 验收（M2-F01 ~ M2-F03）

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
|----|------|----------|----------|------|
| M2-F01 | 前端 — Pipeline Designer UI | Pipeline 可视化设计器（步骤拖拽/连接、属性配置）；Dry-run 预览面板（显示每步输入摘要、预估成本、所需权限）；执行状态监视（实时 JSONL 事件流渲染） | 用户能通过 UI 构建 pipeline 并触发 dry-run；dry-run 结果显示类型错误、权限缺失和成本预估；执行中的 pipeline 状态实时更新 | M2-P05 |
| M2-F02 | 前端 — Agent 会话与 Memory 界面 | Agent Chat 集成（支持 skill 选择、pipeline 触发、approval 交互）；Memory Review UI（查看 due cards、作答 grading、查看 memory items）；Channel 管理面板（连接/断开 IM、查看 channel 事件日志） | 用户能在聊天界面与 Agent 交互并触发 pipeline；memory review 能显示到期卡片并记录 grading；channel 面板能管理飞书连接 | M2-A04, M2-M03, M2-C05 |
| M2-F03 | E2E 验收与 M2 操作手册 | M2 Happy Path E2E（用户 intent → Agent → Pipeline → Execution → Answer → Channel Reply）；拒绝路径回归测试（权限不足、类型错误、approval 拒绝、channel 伪造签名、附件 quarantine 失败、memory conflict）；M2 操作手册 | 全链路 happy path 可重复执行；所有拒绝路径有自动或可重复手动验收；质量门禁全部通过 | M2-F01, M2-F02 |

---

## 推荐执行顺序

按用户选择的「Pipeline + Agent 优先」策略，执行顺序如下：

1. **先完成 M2-P01 ~ M2-P05**：锁定 Pipeline Designer 编译器和真实执行运行时。这是 M2 的核心差异化能力，也是 Agent 和 Channel 的依赖基础。
2. **再完成 M2-A01 ~ M2-A04**：在 Pipeline 运行时之上构建 Agent Runtime 和 MCP 适配，形成完整的「意图 → pipeline → 执行 → answer」闭环。
3. **然后完成 M2-C01 ~ M2-C05**：接入真实 IM（飞书），让外部消息能成为 Agent 的触发源，回复能回到 IM。
4. **接着完成 M2-M01 ~ M2-M04**：在已有 session/wiki/channel 数据流上叠加自动 memory 和 review learning，提升长期智能。
5. **最后完成 M2-F01 ~ M2-F03**：前端配套 UI 和全链路 E2E 验收。

### 关键路径（无并行优化时）

```
M2-P01 → M2-P02 → M2-P03 → M2-P04 → M2-P05
                                              ↓
M2-A01 → M2-A02 → M2-A03 → M2-A04 ────────────┘
                                              ↓
M2-C01 → M2-C02 → M2-C03 → M2-C04 → M2-C05 ─┘
                                              ↓
M2-M01 → M2-M02 → M2-M03 → M2-M04 ────────────┘
                                              ↓
M2-F01 → M2-F02 → M2-F03 ─────────────────────┘
```

### 可并行加速点

- **M2-C01（WASM 插件运行时）可与 M2-P01 并行**：两者不依赖。
- **M2-M01（自动 memory）可与 M2-A01 并行**：agent 骨架和 memory 收集器可同步启动。
- **M2-F01（前端 Pipeline UI）可与 M2-P03 并行**：前端在 mock transport 上开发，等后端 ready 后联调。

---

## 质量门禁

最小门禁（与 M0/M1 保持一致）：

- Rust：`cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。
- TypeScript/Electron：`pnpm typecheck`、`pnpm lint`、`pnpm test`。
- 文档：新增或修改 Markdown 后检查相对链接。
- Git：不得提交 `dist/`、`target/`、`.omx/`、本机系统文件或依赖目录。

M2 新增关键回归测试：

- Pipeline Designer 拒绝类型不匹配、cardinality 冲突和未声明权限的下游连接。
- Pipe Runtime 资源超限（CPU/内存/帧数/超时）触发 `PipelineError::ResourceExceeded` 并终止，不泄漏沙盒外资源。
- Approval Gate 在需要审批的步骤前暂停整个 pipeline；审批通过 resume，审批拒绝触发 compensating action。
- Agent session compaction 保留关键决策点，不丢失未批准的 proposal。
- MCP adapter 调用外部 tool 前通过 policy check；外部 tool 的返回结果带 `taint=untrusted_content`。
- Channel 伪造签名、过期时间戳、重复 event ID 不能进入 ingress；未映射用户不能隐式获得 workspace 权限。
- 远程附件必须经过 quarantine、hash/mime 校验和 malware scan 后才能 `source.add`；失败路径进入审计。
- Memory 与 wiki/source 冲突时自动降级为 `stale`/`hint`，不覆盖权威事实。
- Frozen snapshot 在 session 启动时生成；mid-session memory 写入不热替换当前上下文。
- Review card 的 `stability_days` 调整符合遗忘曲线（again 缩短、easy 延长）。

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| Pipeline Designer 编译器复杂度失控 | 分阶段交付：先支持线性 pipeline，再支持 DAG；filter/map/reduce 表达式先用受限 typed AST，不做通用编程语言。 |
| Agent 调用 LLM 产生幻觉或错误 pipeline | LLM 输出只作为 proposal；Pipeline Designer 做确定性类型检查和权限预估；dry-run 预览让用户确认；approval gate 拦截 side-effect。 |
| MCP 工具引入未审计的 side-effect | `mcp-to-pipe` adapter 必须生成 command manifest 并注册到 Registry；外部 tool 默认 `taint=untrusted_content`；policy 按 manifest 的 `policy_operations` 和 `side_effect_level` 评估。 |
| WASM 插件运行时引入供应链攻击 | 插件 manifest 声明 capabilities 和权限；WASM sandbox 限制文件/网络/环境访问；插件不持有 secret；Core 审计所有 plugin → Bridge 的调用。 |
| IM 事件伪造或重放 | Ingress 层做 signature verification、timestamp window check、event ID dedup；未验证事件不能生成 inert event。 |
| 远程附件成为恶意代码入口 | Quarantine 隔离区与 workspace 分离；下载后计算 observed hash/mime；malware scan（至少 hash/mime 一致性）；PDF active content 检测；全部失败进入审计。 |
| 自动 memory 污染 wiki 权威 | Memory 永远作为 low-trust hint；与 wiki/source 冲突时自动降级；未确认的 memory 不能作为执行依据；injection scan 过滤指令注入。 |
| Pipeline/Agent 前端 UI 与后端状态不同步 | JSONL 事件流驱动前端状态；前端不维护 pipeline 执行状态的唯一副本；断线后支持 replay。 |

---

## 交付物清单

- `seaki-pipeline` crate：Pipeline Designer 编译器（意图解析、类型检查、权限预估、成本估算）。
- `seaki-agent` crate：Agent Runtime（LLM 调用、skills 调度、session compaction、MCP 适配）。
- Pipe Runtime：真实执行引擎（streaming、checkpoint、tee/branch/join、resource limit、per-step policy）。
- MCP 兼容层：`mcp-to-pipe` 和 `pipe-to-mcp` adapter。
- Channel Bridge 运行时：WASM 插件运行时、Secret Broker、Ingress 归一化、Identity Mapping。
- 飞书插件：`plugins/channel/feishu/` protocol adapter。
- Quarantine 管道：远程附件下载、hash/mime 校验、malware scan stub。
- Outbox Dispatcher：lease-based 调度、重试、幂等调和。
- 自动 Memory：`MemoryItem` 状态机、自动收集器、conflict detection、frozen snapshot。
- Review Learning：遗忘曲线调度器、review queue、grading feedback、card generation skill。
- Topic Clustering 与 RunbookIndex：自动聚类、可执行手册索引。
- Electron 前端：Pipeline Designer UI、Agent Chat、Memory Review、Channel 管理。
- E2E 测试：happy path + 拒绝路径回归测试。
- M2 操作手册与架构维护记录更新。

---

## 暂缓到后续阶段（M3+）

- 多平台 sandbox 后端（Linux bubblewrap、Windows）。
- 完整多端（Web、RN、小程序、鸿蒙）。
- 除飞书外的 IM 插件（Slack、企业微信、Discord）。
- Pipeline 分布式执行。
- Agent 自主长期运行（long-running autonomous loop）。
- Memory evolution 自动上线（基础设施就位，实际优化需人工审批）。
- 通用自然语言到任意工具调用的零样本映射。
