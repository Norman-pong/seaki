# 开发环境与质量门禁

[返回架构索引](architecture.md)

本页记录 M0-00 的本地开发入口和 CI 质量门禁。Rust 与前端工程骨架由对应实现任务维护；本页只定义协作时必须保持一致的命令、版本约束和生成物边界。

## 先读文档

开始改动前按顺序读取：

1. `.omx/wiki`：当前仓库未初始化 `.omx/wiki`，因此没有可读 wiki 页面。
2. [架构索引](architecture.md)。
3. [Rust Sandbox Runtime](architecture/sandbox-runtime.md)。
4. [前端抽象](architecture/frontend.md)。
5. [第一阶段任务计划](plans/phase-1-task-plan.md)。

涉及架构、模块边界、权限、安全、数据生命周期、前端契约、插件、管道或 MVP 范围的改动，必须先回到对应权威主题页确认事实来源。

## 工具链基线

### Rust

- 使用 stable Rust toolchain。
- CI 必须安装 `rustfmt` 和 `clippy`。
- 质量门禁：
  - `cargo fmt --check`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo test --workspace`

### TypeScript / Electron

- Node.js 必须满足 Vite 8 要求：`^20.19.0 || >=22.12.0`。
- CI 统一使用 Node.js `22.12.0`，本地开发建议使用 Node 22.12+。
- 包管理器使用 pnpm；CI 安装命令固定为 `pnpm install --frozen-lockfile`。
- M0-00 前端不是无依赖占位，必须提供完整 TypeScript 工具链。
- 前端工具链决策见 [采用 Vite 8 与 Oxc TypeScript 工具链](decisions/0002-frontend-toolchain-vite8-oxc.md)。

M0-00 的前端 `pnpm check` 应聚合这些门禁：

- TypeScript 类型检查。
- Oxc lint：`oxlint`。
- Oxc format check：`oxfmt`。
- Vitest 测试。
- Vite 8 构建或等价的前端编译检查。

## CI

基础 CI 定义在 [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)，包含两个独立 job：

- `Rust`：执行 `cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。
- `TypeScript`：使用 Node.js `22.12.0` 与 pnpm，执行 `pnpm install --frozen-lockfile` 和 `pnpm check`。

当 Rust 或前端 workspace 的实际脚本名称变化时，应优先保持 CI 命令稳定；确需变更时，同步更新本页、[第一阶段任务计划](plans/phase-1-task-plan.md) 和相关决策记录。

## 生成物与提交边界

不得提交这些生成物或本机状态：

- `.omx/`
- `dist/`
- `target/`
- `node_modules/`
- `coverage/`
- `packages/dto/src/generated.ts`（由 `pnpm dto:generate` 从 Rust `seaki-dto` 生成）
- 系统文件和本机编辑器缓存。

当前忽略规则由 [`.gitignore`](../.gitignore) 维护。新增工具产生新的生成目录时，先确认是否属于可重建产物；若是，只补 `.gitignore`，不要把产物纳入版本控制。

### DTO 生成规则

`packages/dto/src/generated.ts` 是 Rust `seaki-dto-codegen` 的生成产物：

- **不要手动修改**。Schema 变更应在 `crates/seaki-dto/src/lib.rs` 中完成，然后运行 `pnpm dto:generate` 重新生成。
- **修改 Rust DTO 后必须重新生成**。`cargo test -p seaki-dto-codegen` 中的 `generated_typescript_is_current` 测试会验证生成文件是否最新；`pnpm dto:check` 也会做相同校验。
- **新 clone 仓库后**，在运行 `pnpm typecheck` 或 `pnpm test` 前，先执行 `pnpm dto:generate` 确保 TypeScript 类型存在。

## 决策记录

- [采用 macOS Seatbelt 作为 M0 主平台 sandbox 后端](decisions/0001-main-platform-sandbox-backend.md)。
- [采用 Vite 8 与 Oxc TypeScript 工具链](decisions/0002-frontend-toolchain-vite8-oxc.md)。
