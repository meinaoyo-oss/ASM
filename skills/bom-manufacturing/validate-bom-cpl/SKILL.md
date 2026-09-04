---
name: validate-bom-cpl
description: 校验 BOM 与贴片坐标文件的位号、数量、封装、面别和坐标一致性，适用于 SMT 发布包验收，不修改输入文件。
metadata:
  category: bom-manufacturing
  mcp_coverage: full
  sources: kicad-happy,easyeda-mcp-pro,eda-agent
---

# BOM 与 CPL 校验

MCP 覆盖：`full`。优先使用 `pcb_validate_bom` 和 `pcb_compare_bom_cpl`，通过 `profile` 明确通用或 JLCPCB 字段规则。

## 工作流

1. 先读取输入文件编码和表头，固定 BOM/CPL 版本与哈希；支持的编码和缺失字段必须记录。
2. 用 `pcb_validate_bom` 检查制造商、MPN、封装、值、数量和位号；发现重复位号、空关键字段或数量冲突时阻断。
3. 用 `pcb_compare_bom_cpl` 比较有效位号集合、DNP、顶/底面、X/Y 坐标、旋转、封装和数量。坐标单位和旋转约定必须显式记录。
4. DNP 料保留在 BOM 的设计记录中，但只有明确标记且不进入装配清单时才可从 CPL 排除；未经规则说明的缺失位号视为错误。
5. 将规则发现和人工判断分别列出；只生成报告，不回写或自动修复 BOM/CPL。

## 输出

返回 `pass`、`warn` 或 `fail`，包含 BOM/CPL 哈希、字段映射、位号差异、数量差异、面别/坐标/旋转问题、使用的 profile、证据路径和未验证项。

