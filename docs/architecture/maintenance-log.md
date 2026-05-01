# 架构维护记录

[返回架构索引](../architecture.md)

## 2026-04-28

- 将单一 `docs/architecture.md` 拆分为架构 wiki 入口页和 `docs/architecture/` 下的主题页。
- 保留 `docs/architecture.md` 作为兼容入口，避免旧链接直接失效。
- 主题页按事实家族分配权威范围：总览、边界、管道、Channel Bridge、记忆、沙盒、前端、路线风险、需求推演、参考溯源。
- 后续风险：若代码实现开始落地，需要把已实现事实与设计假设分离，避免架构草案被误读为当前实现状态。

## 2026-04-28 索引清晰度调整

- 按文档审核意见重写 `docs/architecture.md`：增加一句话定位、架构不变式、五层结构、首次 10 分钟阅读路线、架构深入路线和按任务查找表。
- 新增 `wiki-source-citation.md` 作为 `raw source -> parsed frame -> wiki claim -> citation -> index freshness` 的知识层权威页。
- 将 `sandbox-runtime.md` 中 raw source / parsed frame 语义收敛为指向知识层权威页，本页保留沙盒和 parser 安全约束。
- 明确索引页中的总体架构图和执行链路是导航摘要，权威事实以主题页为准。

## 2026-04-28 主题页标题去编号

- 移除 `docs/architecture/` 主题页标题中的原长文序号，避免分层后的 wiki 页面继续暗示线性阅读顺序。
- 删除索引页中关于“主题页内部编号沿用原始长文”的说明。

## 2026-04-28 参考溯源并入索引

- 将 `docs/architecture/references.md` 的参考表移动到 `docs/architecture.md`，作为索引页附录。
- 删除独立参考主题页入口，减少架构入口中的一次跳转。

## 2026-04-28 第一阶段任务计划

- 新增 `docs/plans/phase-1-task-plan.md`，把 M0 本机纵切拆解为可执行任务、验收标准、质量门禁和风险缓解。
- 新增 `docs/plans/README.md` 作为任务计划索引，避免计划文档混入架构主题页目录。
- 保持 `docs/architecture.md` 只索引架构权威页，不把执行计划列为架构主题页。

## 2026-04-28 M0-00 CI 与决策记录

- 新增 [开发文档](../development.md)，记录 M0-00 本地开发入口、Rust/TypeScript 门禁、CI 命令和生成物边界。
- 新增 [CI workflow](../../.github/workflows/ci.yml)，覆盖 Rust `cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`，以及 Node.js 22.12.0 + pnpm 的 `pnpm install --frozen-lockfile`、`pnpm check`。
- 新增 [ADR-0001](../decisions/0001-main-platform-sandbox-backend.md)，确定 M0 主平台 sandbox 后端为 macOS Seatbelt；执行环境变化时必须更新该决策和相关文档。
- 新增 [ADR-0002](../decisions/0002-frontend-toolchain-vite8-oxc.md)，确定 M0 前端工具链采用 Vite 8、`@vitejs/plugin-react` 6、TypeScript 6、Vitest 4 和 Oxc lint/format。
- 更新 [第一阶段任务计划](../plans/phase-1-task-plan.md) 的 M0-00 行，补齐 CI、开发文档和决策记录交付链接。

## 2026-04-29 M1 任务计划（初稿）

- 新增 `docs/plans/m1-task-plan.md`，把 M1 范围（pipeline inspect/dry-run/compose、session_search + 手动 project note、fake/local channel provider）拆解为可执行任务、验收标准、质量门禁和风险缓解。
- 基于当前代码库状态评估：M0 Rust 后端基本完成，前端 domain/state/dto 基本完成，transport 仅 mock，Electron 为预览级 UI；M1 三大线（pipeline、memory、channel）均无实质代码，需从 crate 骨架开始。
- 保持 `run`、Pipeline Designer 完整功能、自动 memory、复习调度、真实 IM provider 后置到 M2。

## 2026-04-29 M1 任务计划（虚拟需求推演修订版）

- 安排 3 个子代理分别对 Pipeline、Memory、Channel Bridge 线进行虚拟需求链条推演，发现以下关键缺口并修订计划：
  1. **Pipeline 线**：原 M1-01 缺少命令发现能力（`pipe.list`），M1-03 缺少 `PipelineError`/`step.failed` 事件和 `PatchProposalArtifact` 产出，导致 Pipeline 线成为断头路。修订：M1-01 增加 `list` 和 `ParagraphFrame` DTO；M1-03 增加 `PipelineError`、`step.failed`、dry-run 产出 `PatchProposalArtifact` 并接入最小审批链路。
  2. **Memory 线**：M1-04 缺少会话索引触发机制和 redaction 策略；M1-05 的 MemoryItem 生命周期遗漏 `source_checking` 阶段，与 [memory.md](memory.md) 冲突；低信任 data block 注入机制无验证场景。修订：M1-04 明确 daemon 自动触发 redaction pipeline、TTL 清理策略（expired -> 7 天后删除 + audit）；M1-05 恢复 `source_checking` 阶段、增加 note 与 ConceptPage 边界；M1-08 增加"引用历史会话"手动操作以验证注入边界。
  3. **Channel 线**：M1-06 缺少 webhook 验证、`ChannelResourceGrant`、binding 表初始化；M1-07 缺少 `uses_remaining`、FakeProviderQueryAPI、并发 lease 抢占。修订：M1-06 扩展 FakeWebhookVerifier、binding 表 fixture、`ChannelResourceGrant` 与 fake broker quarantine 下载；M1-07 增加 `uses_remaining`、FakeProviderQueryAPI、并发测试；风险缓解声明修改为"M2 替换 provider 并补全真实网络/scale/错误码"，不再过度承诺"只替换实现层"。
- 质量门禁补充：`source_checking` 回归、webhook verify 回归、并发 lease 抢占回归、`ChannelResourceGrant` 签发/消费测试。

## 2026-04-29 M1 任务计划（第二轮虚拟需求推演修订版）

- 安排 3 个子代理进行**第二轮**虚拟需求链条推演，验证第一轮修订是否充分，并发现新问题：
  1. **Pipeline 线**：第一轮 4/5 问题已解决，但修订引入了**结构性矛盾**——M1-01 注册的命令全为 `side_effect_level="none"`，而 M1-03 产出 `PatchProposalArtifact` 要求最后一步为 `proposal_only`，导致 artifact 永远无法触发。此外 `adr.summarize` 语义在架构文档与任务计划间不一致，`wiki.patch.propose` 审批链路后端归属不明确。修订：M1-01 增加 `proposal_only` 命令（`wiki.patch.propose`）；M1-02 区分无副作用链条和 `proposal_only` 链条的验收标准；M1-03 明确 `PatchProposalArtifact` 通过 `wiki.patch.propose` 进入审批链路，复用 M0 已实现的 `WikiPatchTransaction`。
  2. **Memory 线**：第一轮 5/6 问题已解决，但 revision 引入新问题：project note 只有写没有读，"聚合零散笔记"场景存在结构性断点；M1-04 "会话结束时自动触发"在 Electron+mock transport 条件下缺乏真实 session 基础设施；M1-09 低信任注入 e2e 在 mock transport 下不可信；M1-05 对 M0 wiki claim 存在隐式依赖。修订：M1-05 增加 project note 标题+内容关键词 BM25 搜索；M1-04 将触发机制改为"用户手动触发 + daemon 支持手动触发 API"；M1-09 将低信任注入从 e2e 降级为"前端状态测试 + daemon 单元测试"；M1-05 依赖列显式声明 M0-06。
  3. **Channel 线**：第一轮 4/4 问题已解决，但 revision 引入新问题：M1-06 单任务塞入 8 个产出，范围过度膨胀；role-based policy 决策（guest 被拒绝）完全缺失验收；Channel 附件到 wiki 的跨线链路存在结构性缺口（quarantine 为 mock，不进入 `source.ingest`）；IM provenance 未纳入验收。修订：M1-06 拆分为 M1-06a（入站验证 + actor 解析 + role policy）和 M1-06b（附件授权 + quarantine mock）；M1-06a 增加 guest 角色 policy 拒绝验收；M1-07 增加 provenance 字段要求；风险缓解声明补充"M2 补全 Channel 附件从 quarantine 到 `source.ingest` 的真实 sandbox 链路"；M1-06b 诚实声明 quarantine 为契约模拟。

## 2026-04-30 Electron 前端布局重构（M0-09）

- `apps/electron/src/App.tsx`：将 13-panel CSS grid 替换为 `react-resizable-panels` 三列可调整布局（左 18% 会话栏 / 中 50% 聊天区 / 右 32% Wiki 栏）。
- 新增组件：`TitleBar`（macOS 交通灯 + 面板切换）、`SessionSidebar`（会话列表）、`ChatPanel`（消息流 + 输入区）、`WikiSidebar`（Wiki 树 + 预览 + Approval）。
- 面板折叠/展开由 `TitleBar` 图标控制，通过 `usePanelRef` 调用 `panelRef.resize()` 驱动，配合 CSS `transition: flex-basis` 实现抽屉动画。
- 所有自定义 BEM CSS 替换为 Tailwind 工具类 + shadcn/ui 组件（`button`、`badge`、`card`、`textarea`、`tabs`、`separator`）。
- `WikiSidebar` 自研 tab 切换器替换为已安装的 shadcn/ui `Tabs`，获得完整的 `role="tablist"`、`aria-selected`、键盘导航。
- 可访问性修复：`SessionSidebar` 删除按钮添加 `focus-visible:opacity-100`；`ChatPanel` 消息容器添加 `aria-live="polite"`、头像添加 `aria-label`；`ApprovalWidget` 折叠按钮添加 `aria-expanded` + `aria-controls`。
- 性能优化：`ChatPanel` 子组件包裹 `React.memo`；`App.tsx` `useState` 改为惰性初始化；`ChatCardItem` 的 `ICON_MAP` 提升到模块顶层。
- 清理：`ChatSession` 接口移除冗余的 `active: boolean` 派生字段，由单一 `activeSessionId` 状态决定激活会话。
- 验证：E2E 14 passed、oxlint 0 issues、`cargo test` 全部通过。
