# 当前任务状态

> 本文件供 agent 会话快速识别当前工程进度。每次完成一批任务后更新。

## 项目阶段

- **M0（本机纵切）**：✅ 已全部交付（4/28 ~ 4/29）
- **M1（Pipeline / Memory / Channel 骨架）**：✅ 已全部交付（4/29）
- **M1 完成后前端优化迭代**：✅ 已完成（4/30 ~ 5/2）
- **M2（Pipeline / Agent / Channel / Memory 纵切）**：✅ 已全部交付（5/3 ~ 5/6）
- **M2 后续优化（P1-P4 待办）**：✅ 已全部完成（5/12）
- **M3（LLM 接入 → Citation-backed Answer → 飞书闭环）**：✅ 已全部交付（5/12）
- **M4（IPC 桥接 → 流式输出 → 生产可用）**：📋 已规划，待执行

## M3 交付摘要

### Backend（6/6 完成）

| 模块 | 任务 | 状态 |
|------|------|------|
| P1 tokio Runtime | `AgentRuntimeHandle` 封装 | ✅ |
| P2 OpenAiClient | 真实 HTTP 调用、配置、错误映射 | ✅ |
| P3 AnswerComposer | LLM citation-backed answer 生成 | ✅ |
| P4 FeishuProviderDriver | 真实飞书 HTTP 调用、token 管理 | ✅ |
| P5 前端接入 | Citation badge、LLM 模式控制、CommandPalette | ✅ |
| P6 端到端验证 | 660+ Rust tests、111 前端 tests | ✅ |

### Frontend（4/4 完成）

| 任务 | 状态 |
|------|------|
| CitationRef 类型扩展与 badge 可点击 | ✅ |
| `SEAKI_LLM_ENABLED` 环境变量控制 | ✅ |
| `compose-answer` CommandPalette 命令 | ✅ |
| M3 操作手册与回归测试清单 | ✅ |

## M4 规划摘要

### 范围确认

M4 包含 M3 遗留 5 项 + 安全修复 + 测试覆盖 + E2E：

| 阶段 | 内容 | 任务数 |
|------|------|--------|
| P1 安全修复 | 路径遍历、template injection、TOCTOU、WASM 限制、audit 补齐 | 5 |
| P2 IPC 基础设施 | Daemon 生命周期、IPC 协议、transport 实现、API Gateway、连接管理、前端迁移 | 6 |
| P3 运行时配置 | 配置持久化、热重载 API、mock/real 切换 UI | 3 |
| P4 多 Provider | Provider 注册表、动态切换、前端选择器 | 3 |
| P5 流式输出 | stream trait、SSE 解析、事件推送、前端渲染、增量 citation | 5 |
| P6 飞书附件 | 消息解析、下载 Quarantine、Secret Broker 扩展、Outbox 发送、前端展示 | 5 |
| P7 测试覆盖 | 前端零测试补齐、Rust 回归测试、IPC 集成测试 | 3 |
| P8 Playwright E2E | 基础设施、Happy Path、Reject Path | 3 |

**总计：33 个任务，8 个阶段**

### 关键路径

`P1 → P2 → P3 → (P4 + P5 + P6 并行) → P7 → P8`

### 计划文档

- `docs/plans/m4-task-plan.md` — 完整任务计划
- `docs/plans/m4-operation-manual.md` — 待编写（验收手册）

## M2 交付摘要

### Backend（17/17 完成）

| 模块 | 任务 | 状态 |
|------|------|------|
| Pipeline P01~P05 | 意图编译、类型检查、真实执行、DAG 控制流、Approval Gate | ✅ |
| Agent A01~A04 | LLM 调用、Session 管理、Skills 调度、MCP 双向适配 | ✅ |
| Channel C01~C05 | WASM 插件、Ingress、Quarantine、Outbox、飞书插件 | ✅ |
| Memory M01~M04 | 自动收集、Frozen Snapshot、遗忘曲线、卡片生成与聚类 | ✅ |

### Frontend（3/3 完成）

| 任务 | 状态 |
|------|------|
| F01 Pipeline Designer UI（步骤列表、dry-run 预览、事件流） | ✅ |
| F02 Agent Chat + Memory Review + Channel 面板 | ✅ |
| F03 E2E 验收 + M2 操作手册 | ✅ |

### 质量门禁

| 检查项 | 状态 |
|--------|------|
| `cargo fmt --check` | ✅ |
| `cargo clippy --workspace --tests -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 551+ tests, 44 suites |
| `pnpm typecheck` | ✅ |
| `pnpm lint` | ✅ 0 warnings |
| `pnpm test` | ✅ 62 tests |
| `pnpm dto:check` | ✅ |

## 最近完成的迭代（M2 前端配套）

### F01 Pipeline Designer UI
- 新增 `PipelinePanel` / `PipelineStepCard` / `PipelineEventStream`
- ChatPanel header 集成 Pipeline 按钮（Zap 图标）
- Dry-run 预览区：输入摘要、预估成本、所需权限
- 执行状态监视：JSONL 终端风格事件流，`aria-live="polite"`

### F02 Agent 会话与 Memory 界面
- ChatPanel 增强：Skill 选择器（5 个 skill badge）、消息实际发送、Approval 交互按钮
- WikiSidebar 新增 `memory` 和 `channel` tab
- `MemoryReviewPanel`：到期卡片、显示答案、Again/Hard/Good/Easy Grading
- `ChannelPanel`：频道列表（feishu/slack/wecom）、状态 badge、事件日志

### F03 E2E 验收
- 前端组件测试 62 个用例全部通过
- M2 操作手册：`docs/plans/m2-operation-manual.md`
- Happy Path 与 Reject Path 回归测试清单

## 提交记录（按时间顺序，M2 阶段）

```
69146b9  对齐飞书 Open Platform API 的事件格式、加密与签名验证语义
b3e2a9a  实现卡片生成与 Topic Clustering，完成 M2-M04 交付
f74c58c  实现遗忘曲线与复习调度，完成 M2-M03 交付
2f52b50  实现 Memory Frozen Snapshot 与写入管道，完成 M2-M02 交付
830e1c4  实现飞书协议适配器以支持消息收发、附件与线程回复
223b075  实现 Outbox 调度器与真实 Provider 驱动，完成 M2-C04 交付
fb8eaa0  实现自动 Memory 收集基础设施，完成 M2-M01 交付
e7e70d9  实现远程附件导入的 Quarantine 管道与资源授权扩展，完成 M2-C03 交付
1e6b591  实现 Channel Bridge Ingress 归一化与身份映射，完成 M2-C02 交付
d0ab784  实现 Channel Bridge WASM 插件运行时与 Secret Broker，完成 M2-C01 交付
```

## 质量门禁状态（M3 基线）

| 检查项 | 状态 |
|--------|------|
| `pnpm build`（Vite 客户端 + Electron） | ✅ |
| `cargo test --workspace` | ✅ 660+ passed |
| `pnpm dto:check` | ✅ |
| `oxlint src/` | ✅ 0 warnings |
| `pnpm typecheck` | ✅ |
| `pnpm test` | ✅ 111 passed |

## M4 目标门禁

| 检查项 | 目标 |
|--------|------|
| `cargo test --workspace` | 740+ tests |
| `pnpm test` | 160+ tests |
| `pnpm e2e` | Happy Path + Reject Path 通过 |
| 安全审计 | maintenance log 中 Critical + High 项清零 |

## 待办（无阻塞项，均可后续安排）

- [x] `SessionSidebar` 允许删除非活跃会话（当前仅限活跃会话显示删除按钮）✅ 2026-05-12
- [x] E2E 选择器统一为 `data-testid`（当前混用类名和 data 属性）✅ 2026-05-12
- [x] `styles.css` 中 18 个自定义类迁移为纯 Tailwind ✅ 2026-05-12
- [x] Playwright 真实浏览器 E2E — 评估结论：M3 多端开发启动前暂不引入 ✅ 2026-05-12

## 关键文件路径

```
apps/electron/src/App.tsx                    # 根组件（三列布局 + CommandPalette）
apps/electron/src/components/ChatPanel.tsx   # 聊天面板（Skill 选择 + Pipeline）
apps/electron/src/components/WikiSidebar.tsx # 右侧 Wiki 栏（5 Tabs）
apps/electron/src/components/PipelinePanel.tsx       # Pipeline Designer
apps/electron/src/electron/main.ts           # Electron 主进程（M4: daemon 生命周期）
apps/electron/src/electron/preload.ts        # IPC preload（M4: 协议封装）
crates/seaki-pipeline/                       # Pipeline Designer 编译器
crates/seaki-agent/                          # Agent Runtime（M4: stream + 多 provider）
crates/seaki-channel/                        # Channel Bridge + 飞书（M4: 附件）
crates/seaki-daemon/                         # Daemon（M4: API Gateway）
crates/seaki-memory/                         # Memory + Review Learning
crates/seaki-core/                           # Core（M4: 配置持久化）
docs/plans/m4-task-plan.md                   # M4 任务计划
docs/plans/m4-operation-manual.md            # M4 操作手册（待编写）
docs/architecture/                           # 架构文档
```

## 技术栈

- React 19.2.5, TypeScript 6.0.3, Tailwind CSS v4.2.4
- shadcn/ui (radix-nova), Vite 8.0.10, Electron 41.3.0
- `react-resizable-panels@4.10.0`
- Vitest 4.1.5 (unit), oxlint, @testing-library/react
- Rust workspace（13 crates）
