# M0 本机纵切操作手册

[返回任务计划](phase-1-task-plan.md)

本手册记录 M0 端到端验收步骤、质量门禁命令和已知风险清单。M0 交付范围仅限本机纵切：Electron + Rust daemon + 本机 source 导入 + typed wiki page/claim + BM25 candidate search + approval diff + citation-backed answer + citation 回跳。

## 环境准备

```bash
# Rust (stable toolchain)
rustup component add rustfmt clippy

# Node.js 22.12+ 与 pnpm
node --version  # >= 22.12.0
pnpm --version  # >= 9.0.0

# 安装前端依赖
pnpm install --frozen-lockfile
```

## 质量门禁（每次提交前执行）

```bash
# Rust
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

# TypeScript / Electron
pnpm check    # 聚合 dto:check + format:check + lint + typecheck + build + test
```

## Happy Path 演示：source → citation-backed answer

```text
1. 启动 Electron 开发服务器
   pnpm --filter @seaki/electron dev

2. 在 Electron 中初始化 workspace
   - 点击 "初始化 Workspace"
   - 确认 workspace 状态变为 ready

3. 选择本机 Markdown 文件触发 source ingest
   - 选择 .md 文件
   - 观察状态机: selected → grant_requested → granted → raw_committed → parse_running → parsed
   - 确认 source 进入 raw CAS 且 parser 生成 ParsedFrame

4. 审批 wiki patch proposal
   - 查看 ApprovalDiff 屏幕
   - 确认每个 claim 的 citation validation、risk summary 和 taint/security flags
   - 点击 "批准" 或逐条审批 claim
   - 确认 approval 结果进入 WAL/audit

5. 触发索引重建
   - 点击 "重建索引"
   - 确认 index_status 从 stale 变为 fresh

6. 执行搜索
   - 输入查询词
   - 确认 SearchResults 返回 candidate 且包含 citation refs
   - 确认 restricted/tombstoned source 的 snippet 被过滤

7. 点击 citation 回跳
   - 确认 CitationPreview 屏幕显示 source range preview
   - 确认可打开 source range

8. 查看 citation-backed answer
   - 确认 Answer 面板显示 composed text
   - 确认 answer 包含 citation refs
   - 确认 degraded/no_access citation 不生成虚假引用
```

## 拒绝路径验证清单

| 拒绝场景 | 验证命令/步骤 | 期望结果 |
| --- | --- | --- |
| workspace 外路径 | `seaki-policy` 单元测试 | policy 默认拒绝；symlink escape 被拒绝 |
| grant 并发复用 | `seaki-policy` 单元测试 | 过期或 uses 用尽后再次使用失败 |
| PDF 超限 | `seaki-wiki` 单元测试 | 标记 Partial + PdfOversized；不阻塞 raw CAS |
| invalid citation | `seaki-wiki` wiki patch 测试 | WikiPatchTransaction 阻止越权/缺失 citation |
| base revision 冲突 | `seaki-wiki` wiki patch 测试 | 旧 base revision 触发 BaseRevisionConflict |
| tombstoned citation | `seaki-core` m0_reject_path_citation_resolve_returns_no_access_for_tombstoned_source | citation.resolve 返回 no_access |
| index stale | `seaki-core` search_query 测试 | 结果标记 stale；已提交 wiki revision 不被破坏 |
| no_access citation resolve | `seaki-core` m0_reject_path_search_excludes_restricted_candidates_from_authorization | restricted candidate 不进入 authorized results |

运行全部回归测试：

```bash
cargo test --workspace
pnpm test
```

## 已知风险与缓解

| 风险 | 状态 | 缓解 |
| --- | --- | --- |
| AI patch 污染 wiki 权威事实 | 已缓解 | AI 只生成 proposal；commit 必须经过 citation validation、approval、WAL |
| source/index/citation 状态半成功 | 已缓解 | raw CAS、wiki revision、approval、audit 使用 WAL 批次记录 |
| PDF 或外部文本注入越权指令 | 已缓解 | parser 输出永远带 untrusted taint；policy 只读结构化字段 |
| 前端绕过 daemon 权威链路 | 已缓解 | 前端只调用 domain use case；文件/wiki/citation 都回到 daemon |
| 本地索引泄露敏感片段 | 已缓解 | index 只返回 candidate ids；daemon 二次授权后返回 snippet |
| sandbox 平台差异拖慢第一阶段 | 已接受 | 只实现 macOS Seatbelt 主平台后端；第二平台后置到 M1/M2 |

## 交付物检查表

- [x] 可运行的 Electron + Rust daemon 本机开发环境
- [x] Rust crate 与前端包的最小 monorepo 布局
- [x] Workspace 初始化、event replay、audit/WAL 基础实现
- [x] 最小 policy/capability/sandbox/source ingest/wiki transaction/index/search/answer 纵切
- [x] MVP screens：DaemonStatus、WorkspaceShell、ImportQueue、ApprovalDiff、WikiReader、SearchResults、CitationPreview、Answer
- [x] 可重复 demo fixture 和端到端 smoke test
- [x] 与实现同步的架构文档、操作手册和已知风险列表
