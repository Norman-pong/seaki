# 0002 采用 Vite 8 与 Oxc TypeScript 工具链

状态：已接受  
日期：2026-04-28

## 背景

M0-00 的前端工程不是无依赖占位。第一阶段需要冻结 Electron + TypeScript 领域契约、DTO、事件 envelope、状态机和 MVP screen contract，因此前端 workspace 必须从一开始具备可执行的类型检查、lint、格式检查、测试和构建门禁。

## 决策

M0 前端工具链采用 Vite 8 与 Oxc 全套：

| 工具 | 版本基线 | 用途 |
| --- | --- | --- |
| Node.js | CI 使用 `22.12.0`；本地需满足 `^20.19.0 || >=22.12.0` | 满足 Vite 8 runtime 要求 |
| Vite | `8.0.10` | 前端 dev/build 基线 |
| React 插件 | `@vitejs/plugin-react` `6.0.1` | Vite 8 React/Oxc 插件，peer `vite:^8.0.0` |
| TypeScript | `6.0.3` | 类型检查 |
| Vitest | `4.1.5` | 单元测试，支持 Vite 8 |
| Oxc lint | `oxlint` `1.62.0` | lint |
| Oxc format | `oxfmt` `0.47.0` | 格式检查 |

CI 只依赖两个前端命令：

```sh
pnpm install --frozen-lockfile
pnpm check
```

前端 workspace 应让 `pnpm check` 聚合 typecheck、oxlint、oxfmt、Vitest 和 Vite build 或等价编译检查。

## 约束

- 不把 `@vitejs/plugin-react-oxc` 记录为 M0 默认选项；它当前 peer 只到 Vite 7，不满足 Vite 8 默认工具链。
- 不新增无依赖占位前端来绕过门禁。
- DTO 的 schema source of truth 仍在 Rust；TypeScript 侧只消费生成的 DTO 和 schema hash。
- CI 必须继续覆盖 [开发文档](../development.md) 中定义的 `pnpm install --frozen-lockfile` 和 `pnpm check`。

## 后果

M0 会更早暴露 TypeScript、React、Vite、Oxc、Vitest 与 Node 版本不兼容问题。代价是前端 workspace 初始化必须一次性补齐 package、lockfile 和检查脚本；这部分由前端实现任务负责，CI/文档只固定门禁契约。

若 Vite、React 插件、Oxc、Vitest 或 TypeScript 版本发生兼容性变化，必须更新本决策、[开发文档](../development.md) 和 CI。
