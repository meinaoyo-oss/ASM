---
name: review-release-geometry
description: 交叉核对 KiCad PCB 与 Gerber/Excellon 发布几何的板框、铜层、钻孔和焊盘证据，适用于制造发布前审查，不替代 CAM。
metadata:
  category: pcb-design
  mcp_coverage: partial
  sources: kicad-happy,easyeda-mcp-pro,eda-agent
---

# 发布几何一致性

使用 `pcb_geometry_consistency_review`，输入 KiCad PCB/项目和 Gerber/Excellon 发布目录或 ZIP。

## 工作流

1. 固定 KiCad 版本、Gerber/Excellon 发布包版本、profile 和所有输入哈希。
2. 检查 KiCad Edge.Cuts 与 Gerber outline 的边界差异，确认单位、原点、旋转和拼板策略一致。
3. 比较 PCB 与 Gerber 铜层数，比较 PCB 过孔和 Excellon 可解析孔坐标数量；数量差异必须结合通孔焊盘、盲埋孔和工艺说明判断。
4. 检查 PCB pad 与阻焊/铜层闪点证据；闪点计数不是完整 CAM 几何证明，解析失败或没有闪点时保持 `warn/skipped`。
5. 结合 `pcb_dfm_dfa_dft_review`、`pcb_validate_release`、原生 DRC 和制造商 CAM 复核后，才可进入人工发布审批。

## 边界

- 不自动修正坐标、原点、层文件、钻孔或 Gerber；不重打包或上传制造商。
- 不把 outline bounds、flash count 或孔数量一致解释为线路连通、阻焊开窗、阻抗或制造可行性证明。
- ODB++、复杂 arc/polygon/zone 和拼板展开仍需要独立解析器或 CAM 证据。

## 输出

输出输入版本/哈希、PCB 与 Gerber bounds、铜层/钻孔/焊盘指标、漂移项、解析失败、跳过检查和人工复核项。

