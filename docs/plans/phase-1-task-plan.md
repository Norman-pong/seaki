# 第一阶段任务计划：M0 本机纵切

[返回架构索引](../architecture.md)

权威范围：把 M0 架构顺序转成可执行、可验收、可回滚的工程任务计划。本页不替代各主题页的架构事实；当任务计划与主题页冲突时，以主题页为准。

## 阶段目标

第一阶段交付一条本机可演示、可测试、可审计的最小纵切：

```text
本机 source
  -> append-only raw CAS / parsed frames
  -> WikiPatchTransaction
  -> typed wiki page / claim / citation
  -> 本地 BM25 candidate search
  -> citation-backed answer
  -> citation 回跳 source range 或 wiki anchor
```

完成标准：

- 用户能在 Electron 中初始化 workspace、选择本机文件、触发一次性授权、完成 source ingest。
- 系统能把 source 写入 raw CAS，解析为带 range、hash、taint、provenance 的 parsed frames。
- AI 或半自动流程只能生成 wiki patch proposal；权威写入必须经过 policy、approval、WAL 和 `WikiPatchTransaction`。
- 已提交的 typed page、claim 和 citation 能被本地 BM25 搜索命中；index 只返回 candidate ids，daemon 二次授权后才返回 snippet 和 citation refs。
- citation-backed answer 只使用当前 actor 可见的 claim/citation，并能回跳到 source range 或 wiki anchor。
- 所有关键副作用都有 audit/WAL 记录；失败能进入明确的 degraded、stale、denied、failed 或 rollback marker 状态。

## 架构依据

| 依据 | 对任务计划的约束 |
| --- | --- |
| [MVP 顺序与主要风险](../architecture/roadmap-risks.md) | 第一阶段按 M0 顺序收窄为本机纵切，真实 IM、自动 memory、Pipeline Designer 和跨平台 sandbox 后置。 |
| [总览与核心分层](../architecture/overview.md) | 工程拆分围绕 `seaki-core`、`seaki-wiki`、`seaki-policy`、`seaki-sandbox`、`seaki-index`、`seaki-daemon` 和前端包展开。 |
| [边界与权威链路](../architecture/boundaries.md) | 所有入口必须走 `daemon ingress -> inert event -> proposal/plan -> deterministic validation -> policy -> sandbox/broker -> audit/WAL/outbox`。 |
| [Wiki / Source / Citation 知识层](../architecture/wiki-source-citation.md) | raw source、parsed frame、wiki page、claim、citation、index freshness 必须分层建模，index 不能成为事实源。 |
| [前端抽象](../architecture/frontend.md) | 第一阶段冻结 Electron + TypeScript 领域契约、DTO、事件 envelope、状态机和 MVP screen contract。 |
| [Rust Sandbox Runtime](../architecture/sandbox-runtime.md) | 文件读取、source ingest、parser 执行和写入必须通过 capability、policy profile、sandbox enforcement 和 audit。 |

## 非目标

- 不接真实 Channel Bridge、真实 IM provider 或远程附件导入。
- 不实现自动 project/user memory、复习队列、topic clustering 或 review-learning。
- 不实现完整 Pipeline Designer、MCP/skills 兼容层或通用 pipe `run`。
- 不实现跨平台 sandbox；第一阶段只做一个主平台后端，第二平台后置。
- 不做远程 embedding、向量索引或跨 workspace/entity graph 合并。
- 不让前端、插件或 agent 直接读取文件、写 wiki、写 memory、发网络请求或执行命令。

## 任务拆解

| ID | 任务 | 主要产出 | 验收标准 | 依赖 |
| --- | --- | --- | --- | --- |
| M0-00 | 工程骨架与质量门禁 | Rust workspace、前端 workspace、基础 CI 脚本、格式化/测试命令、开发文档 | 空工程能执行格式化、lint/typecheck 和测试占位；生成物不进入 Git | 无 |
| M0-01 | Core workspace、ledger 与 daemon event spine | `seaki-core`、`seaki-daemon`、workspace 初始化、SQLite/WAL/audit、统一 event envelope | `workspace.init()` 能返回 workspace revision、audit head、index status；事件可按 `seq` replay；audit 追加写入 | M0-00 |
| M0-02 | DTO codegen 与前端事件壳 | Rust DTO source of truth、TypeScript DTO 生成、`@seaki/transport` mock/replay、`@seaki/state` task store | schema hash 不一致会失败；mock daemon events 能驱动 AppBoot、Workspace、Import 状态机 | M0-01 |
| M0-03 | 最小 policy 与 opaque capability | `seaki-policy`、路径 canonicalize、allowlist/denylist、一次性 `file.read` grant、approval/audit 模型 | workspace 外路径默认拒绝；symlink escape 被拒绝；grant 只能指定 audience 使用一次，过期或并发复用失败 | M0-01 |
| M0-04 | 主平台 sandbox enforcement | `seaki-sandbox` 主平台后端、`read-only`、`workspace-write`、`source-ingest` profile、parser 运行封装 | `source-ingest` 无网络、只读输入、只写 raw CAS/隔离临时目录；越权写入和网络访问有审计拒绝 | M0-03 |
| M0-05 | Source ingest 与 parsed frames | `seaki-wiki` raw CAS、`SourceManifest`、Markdown parser、PDF text extractor、`ParsedArtifact`、`ParsedFrame` | Markdown/PDF 可进入 `raw_committed -> parse_running -> parsed|partial|failed`；frame 带 source range、text hash、taint、security flags | M0-03, M0-04 |
| M0-06 | Wiki patch transaction 与 typed page | `WikiPatchProposal`、`WikiPatchTransaction`、`ConceptPage` 或 `DecisionRecord`、Claim、CitationRegistry、rollback marker | citation 不存在、越权、tombstoned 或 base revision 过旧时不能 commit；成功 commit 产生新 wiki revision 和 index stale 标记 | M0-05 |
| M0-07 | Approval diff 与 citation evidence picker | `ApprovalRequestDTO`、ApprovalDiff screen、source preview/cited ranges、单条 claim 批准/拒绝、拒绝原因 | 用户能看见 patch diff、claim citation validation、risk summary 和 taint/security flags；审批结果进入 WAL/audit | M0-02, M0-06 |
| M0-08 | 本地 BM25 candidate search | `seaki-index`、index generation、candidate id 查询、daemon 二次授权、`SearchResultDTO` | index 只存可重建派生物；查询先返回 candidate ids；不可见或 tombstoned citation 不会生成 answer context | M0-06 |
| M0-09 | Electron MVP screens | DaemonStatus、WorkspaceShell、ImportQueue、WikiReader、SearchResults、CitationPreview、错误恢复模型 | UI 不把 draft 显示成 committed；断线/刷新可 replay；degraded、stale、no_access、failed 状态可见且可恢复 | M0-02, M0-07, M0-08 |
| M0-10 | Citation-backed answer 与回跳 | answer composer、claim/citation 可见性回查、`citation.resolve()`、source range/wiki anchor preview | answer 必须包含 citation refs；citation 回跳能打开 source range 或 wiki anchor；degraded/no_access 场景不生成虚假引用 | M0-08, M0-09 |
| M0-11 | 端到端验收与发布门禁 | demo fixture、端到端 smoke test、风险回归测试、M0 操作手册 | 本机 source 导入到 citation-backed answer 的 happy path 和关键拒绝路径均可重复执行；所有质量门禁通过 | M0-10 |

## 推荐执行顺序

1. 先完成 M0-00 到 M0-03，锁定工程布局、事件 envelope、DTO 生成和 policy/capability 的安全基线。
2. 再完成 M0-04 到 M0-06，打通 source ingest、parser、raw CAS、parsed frame 和 wiki transaction 的权威状态链。
3. 然后完成 M0-07 到 M0-09，让审批、搜索、reader、citation preview 和错误恢复在 Electron 中可操作。
4. 最后完成 M0-10 到 M0-11，把 citation-backed answer、回跳和端到端验收固化成可重复 demo。

每个任务都按“编码 -> 测试 -> 审阅 -> 修复 -> 提交”闭环推进；如果实现过程中发现架构事实需要调整，先更新对应主题页，再更新本计划和维护记录。

## 质量门禁

最小门禁：

- Rust：`cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。
- TypeScript/Electron：`pnpm typecheck`、`pnpm lint`、`pnpm test`。
- 文档：新增或修改 Markdown 后检查相对链接。
- Git：不得提交 `dist/`、`target/`、`.omx/`、本机系统文件或依赖目录。

关键回归测试：

- policy 拒绝 workspace 外路径、未授权 symlink、过期 grant、重复使用 grant。
- source ingest 失败不能回滚 raw CAS，但必须保留错误摘要和可恢复状态。
- parser 输出全部带 `taint=untrusted_content`，不能升级成 policy、tool instruction 或 system prompt。
- `WikiPatchTransaction` 阻止无效 citation、base revision 冲突和 tombstoned source 新引用。
- index stale 不阻塞已提交 wiki revision，但搜索结果必须显示 stale 状态。
- citation-backed answer 只能使用当前 actor 二次授权后的 claim/citation。

## 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 过早做全量多端或 IM 能力导致 M0 失焦 | 明确 M0 只交付 Electron + Rust daemon 本机纵切；Channel、memory、Pipeline Designer 后置。 |
| AI patch 污染 wiki 权威事实 | AI 只生成 proposal；commit 必须经过 citation validation、approval、WAL 和 rollback marker。 |
| source/index/citation 状态半成功 | raw CAS、wiki revision、approval、audit 和 index stale 使用事务或 WAL 批次记录。 |
| PDF 或外部文本注入越权指令 | parser 输出永远带 untrusted taint；policy 只读结构化字段和授权 scope。 |
| 前端绕过 daemon 权威链路 | 前端只调用 domain use case 和 DTO；文件内容、wiki 写入和 citation resolve 都回到 daemon。 |
| 本地索引泄露敏感片段 | index 只返回 candidate ids；daemon 按 actor、workspace、source status 和 citation visibility 二次授权。 |
| sandbox 平台差异拖慢第一阶段 | 只实现一个主平台后端，抽象 profile 和 policy request，第二平台作为 M1/M2 验证项。 |

## 交付物清单

- 可运行的 Electron + Rust daemon 本机开发环境。
- Rust crate 与前端包的最小 monorepo 布局。
- Workspace 初始化、event replay、audit/WAL 的基础实现。
- 最小 policy/capability/sandbox/source ingest/wiki transaction/index/search/answer 纵切。
- MVP screens：DaemonStatus、WorkspaceShell、ImportQueue、ApprovalDiff、WikiReader、SearchResults、CitationPreview。
- 可重复 demo fixture 和端到端 smoke test。
- 与实现同步的架构文档、操作手册和已知风险列表。

## 暂缓到后续阶段

- M1：`session_search`、手动 project note、pipe inspect/dry-run/compose、fake/local Channel provider。
- M2：真实 Channel Bridge、多 IM 插件、远程附件导入、Pipeline Designer、MCP/skills 适配、跨工具 connector、自动 topic clustering。
