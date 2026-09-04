---
name: validate-pcb-release
description: 对 PCB 制造发布包执行文件集合、Gerber、钻孔、BOM/CPL 和可选 ERC/DRC 门禁检查，适用于打样或量产发布前验收。
metadata:
  category: bom-manufacturing
  mcp_coverage: full
  sources: kicad-happy,easyeda-mcp-pro,eda-agent
---

# PCB 发布包校验

MCP 覆盖：`full`。使用 `pcb_validate_release` 作为主入口，必要时用 `pcb_inspect_package`、`pcb_validate_gerber_set` 和 `pcb_compare_bom_cpl` 定位问题。

## 工作流

1. 明确 `release_kind`（`fabrication` 或 `assembly`）、板层数、制造 profile、项目版本和发布包哈希。
2. 先检查文件角色和集合，再检查 Gerber 层、板框、钻孔、BOM/CPL、必要的说明或版本证据；不按文件名猜测缺失角色。
3. 运行可用的 ERC/DRC。KiCad CLI 缺失或检查被跳过时记录为未覆盖项；关键门禁未验证不得无条件放行。
4. 对每个发现项记录稳定错误码、严重度、证据文件/报告路径和责任人；把制造 profile 的要求与通用格式问题区分开。
5. 源文件只读，报告可写入指定报告目录；不重打包、重命名、修复或上传发布包。

## 放行规则

- `critical` 或 `error` 发现项使状态为 `fail`。
- 仅有可接受的 `warning` 且每项已有批准或关闭依据时，才可标记 `pass-with-warnings`；宿主若只接受 `pass/warn/fail`，映射为 `warn`。
- 任何关键文件缺失、哈希不一致、解析失败或检查跳过都必须在结论中显式列出。

## 输出

输出机器可读摘要、逐项 findings、输入哈希、规则 profile、工具版本、报告路径和人工批准状态，确保另一个 Agent 可复现同一检查范围。
