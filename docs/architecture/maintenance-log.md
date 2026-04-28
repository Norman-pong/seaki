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
