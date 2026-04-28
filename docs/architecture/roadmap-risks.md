# MVP 顺序与主要风险

[返回架构索引](../architecture.md)

权威范围：M0/M1/M2 交付顺序和架构风险清单。

## MVP 顺序

首版 MVP 应收窄为可验证 seaki 差异化价值的本机纵切：`本机 source -> 可审 wiki page/claim -> citation-backed answer -> citation 回跳`。它不追求完整多端、多 IM、多平台 sandbox、自动 memory、真实 Channel provider 或通用 pipeline designer。

MVP / M0 顺序：

1. 单平台 Electron + Rust daemon + workspace 初始化 + SQLite/WAL/audit。
2. 前端事件壳：`@seaki/dto/domain/state/transport`、mock/replay daemon events、基础 screen contracts。
3. `seaki-policy`：最小路径策略、opaque 一次性 `file.read` capability、approval 和 audit。
4. `seaki-sandbox`：先实现一个主平台的最小 enforcement；第二平台后置。
5. `seaki-wiki`：CAS `source.add`、ingest manifest、Markdown parser、PDF text extractor、parsed frames。
6. `WikiPatchTransaction`：base revision、patch diff、claim ids、citation validation、WAL、commit、rollback marker。
7. 最小 `DecisionRecord` 或 `ConceptPage`：人工/半自动创建 typed page block，支持 evidence picker 和 citation。
8. 本地 BM25 search：index 只返回 candidate ids；daemon 二次授权后生成 snippet 和 citation refs。
9. Electron UI：导入队列、approval diff、wiki reader、search results、citation preview、daemon status。
10. citation-backed answer：从已授权 claim/citation 生成回答，并能回跳 source range 或 wiki anchor。

M1 / alpha：

- `session_search` 和手动 project note；自动 user/project memory、复习队列后置。
- `seaki-pipe inspect/dry-run/compose`、JSONL event stream、无副作用 typed pipeline；`run` 等 policy/sandbox/checkpoint 契约稳定后开放。
- fake/local Channel provider 验证 outbox、`ChannelActionGrant` 和 IM provenance，不接真实 IM provider。

M2：

- 真实 Channel Bridge、多 IM 插件、远程附件导入。
- Pipeline Designer、MCP / skills 兼容适配层。
- 跨工具 connector、`RunbookIndex`、自动 topic clustering 和 review-learning。

## 主要风险

- AI 自动写 wiki 造成知识污染：必须依赖引用、diff review、claim confidence、lint 和 rollback。
- 插件权限膨胀：插件只能做协议适配，不能绕过 Rust Core。
- 插件 secret 外带：第三方插件不能持有原始 secret，所有 IM 出站动作必须经 secret broker 和 `ChannelActionGrant`。
- 沙盒语义跨平台不一致：需要以 capability 和 policy request 作为稳定抽象，平台后端只负责尽力执行。
- 本地文件误读误写：默认拒绝 workspace 外路径，外部 source 必须一次性授权。
- IM 场景越权：Channel Event 不能隐式继承本机权限，必须绑定 actor、conversation 和 capability。
- IM 附件越权导入：远程附件必须通过 `ChannelAttachmentRef` 和 `ChannelResourceGrant`，不能按自然语言模糊查找。
- 记忆污染或过期：memory 必须有容量、scope、衰减、来源和删除机制，不能替代 wiki/source。
- 管道组合误伤：Pipeline Designer 必须做类型检查、权限估算和 dry-run，不能把多步副作用藏进一条不可见命令。
- PDF 解析攻击：PDF 解析必须 sandbox、限额、禁用 active content，提取文本只能作为 untrusted source。
- 索引泄露：embedding、实体图和全文索引默认本地生成、加密隔离，远程 embedding 必须显式审批。
