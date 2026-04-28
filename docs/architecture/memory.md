# 记忆系统

[返回架构索引](../architecture.md)

权威范围：记忆分层、遗忘曲线、复习学习技能和记忆演化门禁。

## 记忆系统

seaki 的记忆系统参考 [Hermes Persistent Memory](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/features/memory.md) 的 bounded memory、frozen snapshot、session search 思路，并吸收 [Hermes Agent Self-Evolution PLAN](https://github.com/NousResearch/hermes-agent-self-evolution/blob/main/PLAN.md) 中“评估门禁、人工审核、不可热替换”的约束。记忆不是无限上下文，也不是 wiki 的替代品；它只保存高价值、可压缩、可撤销的行为辅助信息。

### 记忆分层

| 层级 | 用途 | 写入方式 | 上下文注入 |
| --- | --- | --- | --- |
| `user_memory` | 用户偏好、沟通习惯、稳定身份信息 | `memory.propose` -> policy -> audit | 会话开始冻结注入 |
| `project_memory` | 项目约定、环境事实、长期工作流 | `memory.propose` -> source check -> audit | 会话开始冻结注入 |
| `session_search` | 经脱敏的会话索引、摘要和引用回查 | 自动建立 redacted session index，按 TTL / scope 保留 | 不默认注入 |
| `review_memory` | 学习卡片、复习计划、掌握度 | 用户或 agent 提议，用户可编辑 | 只在学习技能中注入 |
| `wiki_claims` | 可引用的项目知识事实 | wiki patch transaction | 不作为 memory 直接注入 |

强制限制：

- Memory 有硬容量上限，超限必须先合并、替换或删除。
- Memory 在会话开始时生成 frozen snapshot，中途写入只持久化，不热替换进当前系统提示。
- Memory 不保存 secret、完整日志、大段代码、原始文档、临时路径或可从 source 重建的事实。
- `session_search` 不保存原始完整 transcript 到 memory store；只能保存脱敏摘要、分片索引、引用指针和可删除的会话索引，必须有 TTL、scope、secret scan 和删除机制。
- Memory 写入必须经过注入攻击扫描、重复检测、scope 绑定和审计。
- Memory 不能覆盖 wiki/source 的权威性；冲突时以 source 和 wiki claim 为准。
- `project_memory` 必须携带 provenance、确认状态和过期策略；未确认或过期内容只能作为提示线索，不得作为自动执行依据。
- memory 注入只能作为低信任 data block，不能进入 system/developer 级指令；涉及权限、命令、路径、secret、审批的 memory 只能作为检索线索，必须回查 source/wiki claim。
- MVP 阶段只启用 `session_search` 和手动确认的轻量 project note；`user_memory`、自动 `project_memory` 和复习调度后置。

`MemoryItem` 生命周期：

```text
proposed
-> scanning
-> source_checking
-> approved | rejected
-> active
-> stale | conflict | expired
-> archived | deleted
```

`MemoryItem` 必须包含 `trust_level`、`confirmed_by`、`source_citation`、`last_verified_at`、`expires_at` 和冲突处理状态。wiki/source 修正后，关联 memory 只能降级为 stale/conflict，不能反向覆盖 wiki。

### 遗忘曲线约束

seaki 使用艾宾浩斯遗忘曲线为记忆和学习内容增加“衰减约束”，避免 agent 把陈旧信息当作永久事实。

基础模型：

```text
retention(t) = exp(-elapsed_days / stability_days)
```

每条可复习记忆记录：

```json
{
  "id": "mem_123",
  "kind": "review_card",
  "scope": "workspace:seaki",
  "content": "seaki 的插件不能直接读写本地文件",
  "source": "docs/architecture/channel-bridge.md",
  "created_at": "2026-04-28T10:00:00Z",
  "last_reviewed_at": "2026-04-28T10:00:00Z",
  "stability_days": 1.0,
  "retention_threshold": 0.72,
  "next_review_at": "2026-04-29T02:00:00Z",
  "review_count": 0
}
```

调度规则：

- 当 `retention(t) <= retention_threshold` 时进入复习队列。
- 用户答对或确认掌握后提高 `stability_days`，答错或犹豫时降低增长幅度。
- 高风险事实的 `retention_threshold` 更高，例如安全边界、部署步骤、审批规则。
- 超过最长未复习时间的 memory 只能作为“可能过期信息”出现，不能作为自动执行依据。
- 反复答错的卡片必须回链 source 或 wiki page，提示重新学习，而不是继续加密集提醒。

### 复习学习技能

seaki 可以提供内置技能 `review-learning`，将 wiki、source、用户笔记或会话摘要转成可复习卡片。

命令示例：

```bash
seaki memory review due --limit 20
seaki memory review answer mem_123 --grade good
seaki memory card create --from wiki/architecture.md --scope workspace:seaki
seaki memory card suspend mem_123
```

技能行为：

- 从 source/wiki 生成短问题、答案、引用和难度。
- 每次复习只展示到期卡片，不刷屏。
- 用户可标记 `again`、`hard`、`good`、`easy`。
- 复习结果只更新 `review_memory`，不会自动改写 wiki。
- 学习技能可用于个人学习，也可用于团队 onboarding、架构约束复盘、安全规则训练。

### 记忆演化门禁

记忆提示、复习策略和技能文本可以优化，但必须走离线评估和审核。参考 Hermes self-evolution 的原则：

- 优化对象可以是记忆提示、复习卡片生成规则、技能说明和 tool description。
- 用户真实记忆内容不可作为可变异对象。
- 候选变体必须通过评估集、回归测试、长度限制、注入扫描和人工审核。
- 任何优化结果只在新会话生效，不能热替换当前会话。
- 每次演化必须保留 lineage、评估分数、失败样例和回滚点。
