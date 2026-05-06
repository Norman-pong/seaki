# 当前任务状态

> 本文件供 agent 会话快速识别当前工程进度。每次完成一批任务后更新。

## 项目阶段

- **M0（本机纵切）**：✅ 已全部交付（4/28 ~ 4/29）
- **M1（Pipeline / Memory / Channel）**：✅ 已全部交付（4/29）
- **M1 完成后前端优化迭代**：✅ 已完成（4/30 ~ 5/2）

## 最近完成的迭代（M1 后前端优化）

### 核心变更
- 13-panel grid → `react-resizable-panels` 三列可调整布局
- 自定义 BEM CSS → Tailwind + shadcn/ui（button/badge/card/textarea/tabs/separator）
- 新增组件：`TitleBar`、`SessionSidebar`、`ChatPanel`、`WikiSidebar`、`CommandPalette`
- 面板折叠/展开（`usePanelRef` + CSS transition 抽屉动画）
- Codex 式交互收敛（中置 CommandPalette，`Ctrl/Cmd+K`，降低边框存在感）

### 可访问性修复
- `focus-visible:opacity-100`（删除按钮键盘可见）
- `aria-live="polite"` + `aria-label`（聊天区）
- `aria-expanded` + `aria-controls`（Approval 折叠按钮）
- shadcn/ui Tabs 替换自研 tab（完整 ARIA）

### 性能优化
- `ChatCardItem` / `ChatMessageItem` 包裹 `React.memo`
- `useState` 惰性初始化 `() => createChatSessions()`
- `ICON_MAP` 提升到模块顶层

### 代码清理
- 移除 `ChatSession.active` 冗余派生状态
- `TreeNodeItem` `expanded` 同步 `node.expanded` prop
- 滚动条样式包裹 `@media (hover: hover)`
- `App.tsx` 删除无意义 `setSessions` 浅拷贝

### 提交记录（按时间顺序）
```
d3f6573  收敛桌面工作台到 Codex 式交互
da38ae8  修复审核发现的代码异味与文档滞后（当前 HEAD）
dffbaff  修正阶段标记 — M1 完成后迭代，非 M0-09
21e276f  移除 ChatSession.active 冗余派生状态
89b5fb7  TreeNode 状态同步、滚动条媒体查询、消息列表 memo
90c975e  三列可调整布局重构与可访问性修复
e78ad72  参照 AI 工作台设计图升级视觉与交互细节
1a8f30a  将 MVP 网格改造为 Trae Solo 风格的三栏 AI Wiki 工作站
```

## 质量门禁状态

| 检查项 | 状态 |
|--------|------|
| `pnpm build`（Vite 客户端 + Electron） | ✅ |
| `pnpm e2e`（Playwright） | ✅ 15 passed |
| `cargo test --workspace` | ✅ 191 passed |
| `pnpm dto:check` | ✅ |
| `oxlint src/` | ✅ 0 warnings |

## M2 阶段计划（已制定，Pipeline + Agent 优先）

详见 [`docs/plans/m2-task-plan.md`](plans/m2-task-plan.md)。

**执行策略**：Pipeline + Agent 优先，Channel Bridge 次之，Memory 智能化最后。

| 阶段 | 任务范围 | 状态 |
|------|----------|------|
| 阶段一 M2-P | Pipeline Designer + Pipe Runtime（真实执行） | 🔄 待启动（M2-P01） |
| 阶段二 M2-A | Agent Runtime + MCP 适配 | ⏳ 等待 P 阶段完成 |
| 阶段三 M2-C | Channel Bridge 真实化 + 飞书插件 | ⏳ 等待 A 阶段完成 |
| 阶段四 M2-M | 自动 Memory + Review Learning | ⏳ 等待 C 阶段完成 |
| 阶段五 M2-F | 前端配套 + E2E 验收 | ⏳ 等待后端完成 |

## 待办（无阻塞项，均可后续安排）

- [ ] `SessionSidebar` 允许删除非活跃会话（当前仅限活跃会话显示删除按钮）
- [ ] E2E 选择器统一为 `data-testid`（当前混用类名和 data 属性）
- [ ] E2E 增加 `test.beforeEach` 前置条件显式导航
- [ ] `styles.css` 中 `.app-shell` / `.app-body` / `.app-panels` 等自定义类迁移为纯 Tailwind（非阻塞建议）

## 关键文件路径

```
apps/electron/src/App.tsx                    # 根组件（三列布局 + CommandPalette）
apps/electron/src/components/ChatPanel.tsx   # 聊天面板
apps/electron/src/components/SessionSidebar.tsx  # 左侧会话栏
apps/electron/src/components/WikiSidebar.tsx  # 右侧 Wiki 栏（含 Tabs）
apps/electron/src/components/TitleBar.tsx    # 标题栏
apps/electron/src/components/CommandPalette.tsx  # 命令面板
docs/plans/phase-1-task-plan.md              # 阶段任务计划（含执行记录）
docs/architecture/maintenance-log.md         # 架构维护记录
docs/architecture/frontend.md                # 前端抽象（已更新 Screen Contracts）
```

## 技术栈

- React 19.2.5, TypeScript, Tailwind CSS v4.2.4
- shadcn/ui (radix-nova), Vite 8.0.10, Electron 41.3.0
- `react-resizable-panels@4.10.0`（exports `Group`/`Separator`/`Panel`/`usePanelRef`）
- Playwright 1.59.1 (E2E), Vitest 4.1.5 (unit), oxlint
