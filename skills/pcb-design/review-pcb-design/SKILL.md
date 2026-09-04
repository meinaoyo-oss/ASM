---
name: review-pcb-design
description: 审查 PCB 设计发布包的结构、制造层和 ERC/DRC 证据，适用于设计评审与打样前检查，不替代人工签核或实验室验证。
metadata:
  category: pcb-design
  mcp_coverage: partial
  sources: kicad-happy,easyeda-mcp-pro,eda-agent
---

# PCB 设计审查

MCP 覆盖：`partial`。可调用 `kicad_inspect_design`、`kicad_design_review`、`kicad_semantic_review`、`kicad_connectivity_review`、`kicad_schematic_pcb_consistency`、`kicad_power_tree_review`、`kicad_trace_signal`、`kicad_compare_revisions`、`pcb_inspect_package`、`pcb_validate_gerber_set` 和 `pcb_run_kicad_checks`；真实 KiCad 库符号引脚映射、信号完整性、EMC 和布局经验判断仍需外部证据。

## 工作流

1. 先确认项目版本、输入路径和发布包哈希；不要把未确认的临时导出物当作正式设计。
2. 用 `kicad_inspect_design` 读取原生工程对象，再用 `pcb_inspect_package` 识别发布文件角色，按预期铜层数检查 Gerber、钻孔、板框、阻焊和丝印集合。
3. 使用 `kicad_semantic_review`、`kicad_connectivity_review`、`kicad_power_tree_review` 和 `kicad_trace_signal` 做结构化初审；连通性结果只对全局坐标可识别的 pins/wires 有效，库定义中的局部 pin 坐标不能直接当作实例连线。能使用 KiCad CLI 时再执行 ERC/DRC。工具缺失、跳过或版本不明必须保留为未验证项，不能当作通过。
4. 运行 `kicad_design_review` 汇总元件合同、电源、连通性、时钟/复位和接口命名规则，查看各 check 的覆盖范围后再下结论。
5. 对同时存在原理图和 PCB 的工程运行 `kicad_schematic_pcb_consistency`，核对位号集合、封装、pin/pad 编号和有标签网络；没有 pin-to-net 证据时保留为未验证项。
6. 将工具结果与工程判断分开，按 `critical/error/warning/info` 归类，并给每条结论附文件、规则或报告证据。
7. 仅输出审查结论和整改建议，不直接修改源工程、重命名生产文件或放行制造。

## 重点判断

- 检查板框闭合、铜层连续编号、机械层用途、钻孔与焊盘关系，以及丝印是否可能覆盖焊盘。
- 检查 ERC/DRC 的错误是否被规则豁免；每个豁免必须有编号、理由和责任人。
- 区分“未检测到问题”和“没有可用检测证据”，后者至少为 `warn`。

## 输出

返回 `pass`、`warn` 或 `fail`，并列出设计版本、检查范围、发现项、证据路径、未覆盖项目和人工签核人。任何关键检查缺失时不得输出无条件 `pass`。
