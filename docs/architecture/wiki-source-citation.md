# Wiki / Source / Citation 知识层

[返回架构索引](../architecture.md)

权威范围：raw source CAS、source manifest、parsed artifact/frame、wiki page/block/schema、claim、citation registry、wiki patch transaction、index freshness 与 citation-backed answer 的权威关系。

## 职责边界

知识层负责把原始资料和派生知识分开建模：

- `raw source` 是唯一源材料，append-only、content-addressed、可 tombstone，但不能原地覆盖。
- `parsed artifact/frame` 是可重建派生物，用于摘要、索引、wiki patch 和 citation 定位。
- `wiki page/block/schema` 是人工可维护的知识层，不直接替代 source。
- `claim` 是可引用的事实或判断，必须绑定 citation validation。
- `citation registry` 负责把 claim、wiki page、source range 和可见性校验连起来。
- `index` 只保存可重建候选，不作为事实源；回答前必须回查 source、claim 和 citation 权威状态。

不属于本页权威范围：

- 文件、命令、网络、secret 的实际 enforcement，见 [Rust Sandbox Runtime](sandbox-runtime.md)。
- “谁能决定什么”的全局安全边界，见 [边界与权威链路](boundaries.md)。
- 前端 DTO、状态机和展示恢复，见 [前端抽象](frontend.md)。
- pipeline、MCP adapter 和工具组合协议，见 [管道命令接口](pipeline.md)。

## 核心对象

| 对象 | Owner | 关键约束 |
| --- | --- | --- |
| `SourceManifest` | `seaki-wiki` | 记录 source 生命周期、origin display、mime、size、permission scope、parse status、tombstone 和 visibility |
| `ParsedArtifact` | `seaki-wiki` | parser run 的可重建产物，绑定 source hash、parser version、security flags 和生成时间 |
| `ParsedFrame` / `ParagraphFrame` | `seaki-wiki` | 最小可引用解析片段，包含 source range、text hash、taint、trust level 和 schema hash |
| `WikiPage` | `seaki-wiki` | 人工维护的派生知识页，按 schema 存储，不直接成为 raw source |
| `Claim` | `seaki-wiki` | 可引用事实或判断，必须携带 citation、confidence、supersede/conflict 状态 |
| `Citation` | `seaki-wiki` | 指向 `source_id + range`、frame 或 wiki anchor，必须可做权限与 tombstone 校验 |
| `CitationRegistry` | `seaki-wiki` | claim/page 到 source range 的权威映射和 validation 状态 |
| `WikiPatchProposal` | `seaki-wiki` / `seaki-agent` | agent 或 pipeline 只能生成 proposal，包含 base revision、diff、claim ids、citation validation 和 risk summary |
| `WikiPatchTransaction` | `seaki-wiki` / `seaki-core` | 唯一 patch apply 入口，负责 WAL、citation validation、commit、rollback marker 和 index stale 标记 |
| `IndexGeneration` | `seaki-index` | 可重建索引代次，记录 schema version、覆盖的 wiki/source revision、fresh/stale/failed 状态 |

## Source Ingest 生命周期

`source.ingest` 必须使用稳定状态机：

```text
selected
-> grant_requested
-> granted | capability_denied
-> raw_committed
-> parse_running
-> parsed | partial | failed
-> patch_proposed
-> approval_pending
-> committed | denied
-> indexed | index_stale
```

约束：

- `raw_committed` 只说明原始 blob 已进入 CAS，不代表解析、wiki patch 或索引完成。
- `parsed|partial|failed` 只说明解析产物状态；解析失败不能回滚 raw CAS，但必须记录错误摘要。
- `patch_proposed` 仍是 proposal，不是 wiki 权威事实。
- 只有 `WikiPatchTransaction` commit 后，wiki revision、claim 和 citation 才成为权威状态。
- `indexed` 是派生状态；index rebuild 失败只能标记 `index_stale`，不能破坏已提交 wiki transaction。

## Raw Source 存储协议

- `raw/` 只能通过 `source.add` 写入，不作为普通可写目录暴露给工具。
- source blob 使用内容寻址，但存储 key 应使用 per-workspace keyed digest，例如 `HMAC(workspace_key, content_hash)`，避免裸 hash 泄漏“是否导入过某文件”。
- ingest manifest 默认记录 `origin_display`、mtime、mime、size、source id、导入 actor 和时间；完整原始路径只能加密存储或短期保留，不写入普通日志。
- 不允许原地覆盖；删除只能 tombstone，不能破坏已有 citation。
- wiki citation 对外指向稳定 `source_id` 加 byte/page/line range，内部再映射到 raw hash 和 keyed storage key。

## Parsed Frame 协议

- Markdown、PDF、Office 等 source 必须先解析为 content-addressed parsed frames，再进入摘要、索引或 wiki patch。
- parsed frame 必须包含 `source_id`、内部 source hash、parser version、page/line/byte range、mime sniff 结果、文本片段 hash、trust level、taint 和 security flags。
- parsed artifact 是可重建派生物；原始 raw blob 仍是唯一源材料。
- 解析失败不能阻塞 raw CAS 写入，但必须标记 source 的 parse status 和错误摘要。
- PDF、Office、网页和 IM 附件提取出的文本永远是 untrusted source content，不能被当作 system prompt、tool 指令或 policy 指令。

## Wiki Patch Transaction

wiki 写入必须优先走 patch transaction：

```text
propose patch -> policy check -> optional approval -> write WAL -> apply -> commit -> rebuild/update derived index
```

事务边界：

- wiki、source manifest、memory、approval 和 audit log 的权威状态必须同事务或同 WAL 批次提交。
- wiki patch 必须包含 `patch_id`、`base_revision`、diff、claim ids、citation validation result、actor、policy decision、rollback marker 和 transaction id。
- citation validation 必须确认每个引用都指向存在且未越权的 `source_id + range`，内部再校验 source hash / keyed storage key。
- tombstoned source 的既有 citation 可保留审计可见，但默认不可用于新回答；UI 应显示 degraded citation。
- index、搜索缓存、向量库和实体图是可重建派生物，不能作为唯一权威记录。

## Index 与回答权限

- index 查询只能返回 candidate ids。
- daemon 必须按当前 actor、workspace、thread/channel scope、source status 和 citation visibility 做二次授权后，才能生成 snippet、citation refs 或 answer context。
- source tombstone 或权限收回后，索引必须同步清理或标记不可检索。
- entity graph 不能跨 workspace 自动合并身份、组织、文件或 channel 关系。
- 默认使用本地 embedding；远程 embedding 必须单独审批、审计并显示数据范围。

## Schema 化页面类型

M0 可以从少量 typed page block 开始：

- `DecisionRecord`：背景、选项、否决理由、决策人、日期、证据引用和复审条件。
- `ConceptPage`：概念定义、source card、annotation、temporal context、claim supersede。
- `RunbookIndex`：服务、告警、关联代码、回滚手册、历史事故和只读跨工具链接。

这些页面类型属于 `seaki-wiki/schema`，必须以 source、claim 和 citation 为权威，不自动写入长期 memory。
