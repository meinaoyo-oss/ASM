---
name: prepare-jlcpcb-package
description: 按 JLCPCB 装配字段和文件要求准备并预检 PCB 制造包，适用于导出前整理，不执行上传、下单或替换物料。
metadata:
  category: bom-manufacturing
  mcp_coverage: partial
  sources: kicad-happy,easyeda-mcp-pro
---

# JLCPCB 发布包准备

MCP 覆盖：`partial`。MCP 可用 `profile: jlcpcb` 预检 BOM/CPL 和发布集合，但不覆盖厂商网页实时规则、报价、上传、下单和工艺确认。

## 工作流

1. 固定项目版本、板层数、板框、制造选项和目标装配 profile；确认 JLCPCB 当前要求的文档版本来自官方页面或受控记录。
2. 准备 Gerber、钻孔、BOM、CPL 和必要的说明文件；字段映射必须保留原始位号、MPN、封装、数量、DNP、面别、坐标单位和旋转定义。
3. 运行 `pcb_validate_release`、`pcb_validate_bom` 和 `pcb_compare_bom_cpl`；先修正阻断错误，再处理告警。
4. 核对需要人工确认的事项：可装配封装、极性/Pin 1、特殊工艺、拼板、替代料和 DNP 处理。不能仅凭文件通过这些事项。
5. 输出待上传目录或归档包清单与哈希；除非用户另行授权，不连接账户、不上传、不下单、不修改源工程。

## 边界

- 不把社区脚本或旧版网页字段当成厂商当前规则；规则来源和检查时间必须写进报告。
- 不自动选库存料、自动接受替代料、自动配置价格或生产工艺。
- 本 Skill 生成的报告和清单保持 UTF-8 无 BOM、LF；制造文件保持原始格式，并避免路径穿越或无关文件。

## 输出

输出 JLCPCB profile、文件清单、哈希、BOM/CPL 差异、待人工确认项和上传前阻断项，并明确 MCP 未覆盖的在线步骤。
