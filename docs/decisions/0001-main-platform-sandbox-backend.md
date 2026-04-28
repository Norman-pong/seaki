# 0001 采用 macOS Seatbelt 作为 M0 主平台 sandbox 后端

状态：已接受  
日期：2026-04-28

## 背景

M0 只实现一个主平台 sandbox 后端，第二平台后置。当前第一阶段目标是打通 Electron + Rust daemon 的本机纵切，并把文件读取、source ingest、parser 执行和写入收束到 capability、policy profile、sandbox enforcement 和 audit 链路。

[Rust Sandbox Runtime](../architecture/sandbox-runtime.md) 已把 macOS 的推荐后端定义为 Seatbelt (`/usr/bin/sandbox-exec`)。

## 决策

M0 主平台 sandbox 后端采用 macOS Seatbelt，执行入口固定使用 `/usr/bin/sandbox-exec`，不从 `PATH` 查找。

M0 的 `seaki-sandbox` 后端实现应围绕这些 profile 验证：

- `read-only`
- `workspace-write`
- `source-ingest`

`source-ingest` 必须默认无网络、只读输入 blob、只写 raw CAS 或隔离临时目录，并产生可审计的拒绝或失败摘要。

## 约束

- AI 只提交 intent、plan 草案或 patch；Rust Core 和 policy 才能裁决副作用。
- 所有路径必须先 canonicalize，再检查 allowlist / denylist。
- loopback proxy、Unix socket、daemon admin socket、Docker socket、SSH agent 和 cloud credential sockets 默认拒绝，除非后续 broker 明确授权。
- `danger-full-access` 只可作为显式开发调试模式，不作为产品默认模式。

## 后果

M0 可以先验证 macOS 本机开发体验和 sandbox policy transform，避免第一阶段被多平台差异拖慢。

Linux、Windows 和 WSL2 后端仍保留在架构设计中，但不进入 M0 实现范围。若实际执行环境从 macOS 变为 Linux、Windows、WSL2 或 CI 需要运行 sandbox enforcement 测试，必须更新本决策、[Rust Sandbox Runtime](../architecture/sandbox-runtime.md)、[开发文档](../development.md) 和 [第一阶段任务计划](../plans/phase-1-task-plan.md)。

## 未选择

- Linux bubblewrap + seccomp：保留为后续平台后端；M0 不以它作为主平台。
- Windows restricted-token / AppContainer 风格隔离：保留为后续平台后端。
- 纯应用层策略模拟：不能证明子进程文件、网络和 socket 约束，因此不作为 M0 主后端。
