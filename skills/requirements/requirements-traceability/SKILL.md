---
name: requirements-traceability
description: 解析和审查工程需求，建立需求到设计、测试、制造对象的可追溯矩阵，并分析变更影响；适用于需求基线和 ECO 评审，不自动批准变更。
metadata:
  category: requirements
  mcp_coverage: full
  sources: kicad-happy,eda-agent
---

# 需求追溯与变更影响

MCP 覆盖：`full`。使用 `requirements_ingest`、`requirements_quality_review`、`requirements_traceability` 和 `requirements_change_impact`。支持 JSON、CSV，以及包含 `REQ-*`/`SYS-*` 标识的 Markdown。

## 工作流

1. 固定需求文件版本和 SHA-256；确认每条需求有稳定 ID、明确陈述、生命周期状态和验证方法。
2. 先运行 `requirements_quality_review`，修复重复 ID、空陈述和已批准需求缺少验证方法的问题。
3. 用独立 JSON/CSV 链接文件建立需求到设计、测试、制造对象的关系；每条链接保留关系类型和证据路径。
4. 运行 `requirements_traceability`，区分已覆盖、未覆盖、未知需求和无证据链接；不要把“存在链接”当作验证通过。
5. 需求变化时运行 `requirements_change_impact`，输出受影响对象、相关需求和待重新验证项，再由工程/质量责任人批准 ECO。

## 边界

- 不从自然语言相似度猜测追溯关系；没有明确链接或来源证据时标记为未覆盖。
- 不修改原始需求、设计文件、测试结果或 PLM 基线。
- MCP 只提供本地结构化分析；正式基线、配置项状态和批准记录仍以 PLM/QMS 为准。

## 输出

输出需求版本和哈希、质量发现、覆盖率、未覆盖需求、未知链接、追溯目标、受影响对象、证据路径和人工批准状态。
