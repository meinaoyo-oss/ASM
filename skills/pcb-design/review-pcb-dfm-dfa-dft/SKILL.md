---
name: review-pcb-dfm-dfa-dft
description: 对 KiCad PCB 进行制造性、可装配性和可测试性预检，检查板框、层、线宽、过孔、焊盘边距、封装和测试入口；不替代 CAM 审核或工厂能力确认。
metadata:
  category: pcb-design
  mcp_coverage: full
  sources: kicad-happy,easyeda-mcp-pro,eda-agent
---

# PCB DFM/DFA/DFT 预检

使用 `pcb_dfm_dfa_dft_review`，并在发布前结合 `kicad_schematic_pcb_consistency`、`pcb_validate_gerber_set` 和 `pcb_validate_release`。

## 工作流

1. 固定 KiCad PCB 版本、制造 profile、板层数和工厂规则来源；报告中的 `generic`/`jlcpcb` 阈值只代表本地预检。
2. 检查 Edge.Cuts 是否存在并形成闭合端点图，确认 F.Cu/B.Cu 和预期内层集合。
3. 检查所有走线宽度、过孔 size/drill、焊盘到板边距离和封装 pad；发现错误先阻断发布，告警要绑定人工关闭依据。
4. 检查元件原点重叠候选、缺少 pad 的封装和没有明显测试入口的板；这些结果需要结合封装图、探针方案和工艺能力复核。
5. 运行原生 KiCad DRC/ERC 与独立 Gerber/IPC-2581 检查；任何跳过项必须明确写入证据，不得因静态预检通过而放行。

## 边界

- 不自动修改 PCB、重布线、调整线宽、移动器件、生成测试点或上传工厂。
- 不把本地阈值当作 JLCPCB/PCBWay 或其他工厂当前能力；实时规则以受控厂商资料和 CAM 结果为准。
- 未能解析的几何或 pad 坐标保留为 `warn/skipped`，不能默认为通过。

## 输出

输出 profile、阈值、板框/层/走线/过孔/焊盘/封装/测试入口指标、finding code、证据路径、跳过检查和人工关闭项。

