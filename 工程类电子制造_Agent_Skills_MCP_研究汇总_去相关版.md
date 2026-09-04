# 工程类与电子制造 Agent：Skills 与 MCP 接入研究汇总（聚焦通用工程与制造）

> 本版基于原报告整理，聚焦通用工程研发、PCB、嵌入式、测试测量、自动化、制造现场、质量与供应链，保留原有评级、架构和来源链接。

# 全网复查后的结论

本报告不再局限于 PCB/CAD，而是覆盖**需求管理、系统工程、电子设计、嵌入式、仿真、测试测量、PLC/SCADA、MES、设备联网、质量、供应链和生产追溯**。

截至 **2026 年 8 月 30 日**，工程领域的 Agent 接入已经从“社区做几个 CAD 自动化脚本”发展成四个层次：

1. **官方 MCP + 官方 Skills**：MathWorks、NVIDIA 等开始同时提供工具接口和工程方法。
2. **官方 MCP 产品**：Autodesk、Ansys、CODESYS、Keysight、Jama、IBM、Tulip、HighByte 等已经发布产品或公开预览。
3. **行业标准之上的 MCP**：OPC UA、IPC-CFX、Hermes、MTConnect、IVI/VISA/SCPI 等比单一软件插件更适合电子制造。
4. **社区 MCP**：KiCad、Altium、EasyEDA、SPICE、FreeCAD、SolidWorks 等已有大量项目，但成熟度差异很大。

我的总体判断是：

> **目前最成熟的工程 Agent 接入点，不是自由操作 CAD，而是需求追溯、仿真验证、设计审查、PLC 工程、仪器测试、工业数据查询和制造发布检查。**

直接修改 PCB、直接控制设备、自动投产和自动下单，仍应放在最后阶段。

另外，一个刚出现的重要变化是：Anthropic 于 **2026 年 8 月 27 日**公开了 **Model Hardware Standard，MHS** 研究预览，目标是让 Agent 以标准方式安全操作显微镜、液体处理设备、机械臂等物理设备。它可能成为 MCP 从“软件工具”走向“物理设备”的补充，但目前仍只是面向首批科研机构和先进制造企业的研究预览，不能按成熟工业标准使用。([anthropic.com](https://www.anthropic.com/news/model-hardware-standard-research-preview?utm_source=chatgpt.com))

---

# 一、这次采用的成熟度口径

下面的评级是**工程落地评级**，不是厂商官方评级：

| 评级 | 含义 |
|---|---|
| **A** | 官方或标准组织发布，有权限、审计、验证或明确产品支持，可进入受控生产试点 |
| **B** | 官方产品但较新、Public Preview、Labs，或功能范围有限 |
| **C** | 社区项目，可用于实验和内部提效，不建议直接修改生产资产 |
| **I** | 尚无成熟 MCP，但已有稳定 API、CLI 或工业协议，适合自建“薄 MCP” |

能力标签：

- **R**：读取、搜索、追踪、问答
- **V**：仿真、分析、验证、诊断
- **W**：修改工程文件或系统数据
- **X**：控制仪器、PLC、机器人或生产设备

即使评级为 A，只要涉及 **X 物理执行**，也不代表可以绕过安全 PLC、设备联锁或人工批准。

---

# 二、官方或标准组织支持度最高的工程软件

## 1. 系统设计、仿真、CAD、CAE

| 平台 | Agent 能力 | 状态与评级 |
|---|---|---|
| **MATLAB Agentic Toolkit** | 运行 MATLAB、生成和测试代码、诊断错误、使用工具箱、构建应用 | **A，R/V/W**。官方同时提供 MATLAB MCP Core Server 和 Skills Catalog，是目前“工具接口 + 工程 Skills”组合最完整的平台之一。([mathworks.com](https://www.mathworks.com/products/matlab-agentic-toolkit.html?utm_source=chatgpt.com)) |
| **Simulink Agentic Toolkit** | 读取和修改模型、运行仿真、调试、测试、需求追踪、使用 Stateflow、Simscape、System Composer | **A-，R/V/W**。官方明确采用 MCP，并提供面向 Model-Based Design 的 curated skills。([mathworks.com](https://www.mathworks.com/products/simulink-agentic-toolkit.html?utm_source=chatgpt.com)) |
| **Ansys PyAnsys MCP 系列** | 结构、热、流体、电磁、光学和多物理场模型的创建、求解、参数扫描和结果查询 | **B+，R/V/W**。官方 PyAnsys 体系已列出 PyMechanical、PyFluent、PyAEDT、PyMAPDL、PyLumerical、PyCFX 等多个 MCP 项目，但整体仍比较新。([github.com](https://github.com/ansys/pyansys)) |
| **Autodesk Fusion MCP** | 创建特征、修改几何体、执行设计和制造工作流 | **A-，R/V/W**。Fusion MCP 和 Fusion Data MCP 已进入官方 GA；数据 MCP 还可访问项目、Hub、文件夹和权限。([help.autodesk.com](https://help.autodesk.com/view/ADSKMCP/ENU/)) |
| **AutoCAD、Civil 3D、Revit MCP** | 查询图纸、模型对象和部分工程数据 | **B，主要 R，有限 W**。AutoCAD/Civil 3D 和 Revit 的官方 MCP 仍属于 Public Beta，不应与 Fusion GA 混为一谈。([help.autodesk.com](https://help.autodesk.com/view/ADSKMCP/ENU/)) |
| **Onshape FeatureScript MCP** | 由自然语言生成 FeatureScript，创建可重用建模特征 | **B，R/W**。属于 Onshape Labs，适合参数化特征生成，不等同于完整控制所有 Onshape 装配和 CAD 操作。([ptc.com](https://www.ptc.com/en/news/2026/onshape-launches-featurescript-mcp-server?srsltid=AfmBOopxuXp5PDyR-mzukpPa9LDl6G5vd6Bb7Oqew6y9gF3gQNBaHZHK&utm_source=chatgpt.com)) |
| **NVIDIA Isaac Sim MCP + Skills** | 启动或连接仿真、修改 USD 场景、运行代码、操作 UI、构建设备或机器人仿真 Skill | **A-/B+，R/V/W**。适合数字孪生、机器人仿真和合成数据，不应直接替代现实机器人安全控制。([forums.developer.nvidia.com](https://forums.developer.nvidia.com/t/isaac-sim-6-0-general-availability/372621)) |
| **Mastercam Copilot** | 调整进给速度、辅助创建机床组和 CAM 操作 | **非 MCP，属于原生 AI**。可作为 Agent 架构中的原生工具，但不能因为具备 Copilot 就视为开放 MCP。([mastercam.com](https://www.mastercam.com/solutions/add-ons/mastercam-copilot/?utm_source=chatgpt.com)) |

这一类中，**MathWorks、Ansys 和 Autodesk Fusion** 是现阶段最值得作为通用工程 Agent 核心的三套平台。

---

# 三、需求、系统工程、PLM 和数字线程

这部分此前很容易被忽略，但事实上比自动点击 CAD 更适合 Agent。

| 平台 | Agent 能力 | 状态与评级 |
|---|---|---|
| **Jama Connect MCP Server** | 查询需求、测试、风险、基线、关系和追溯图；遵循 Jama 权限和生命周期流程 | **A-，R/有限 W**。官方于 2026 年发布，强调权限、审计和工程追溯，是目前最成熟的需求工程 MCP 之一。([jamasoftware.com](https://www.jamasoftware.com/press/jama-software-launches-model-context-protocol-mcp-server/)) |
| **IBM Engineering AI Hub** | 通过托管 MCP 访问 ELM 和 Rhapsody Systems Engineering 数据及操作 | **A-/B+，R/W**。适合需求、SysML、系统架构和变更影响分析。([ibm.com](https://www.ibm.com/docs/en/engineering-ai-hub/1.3.0?topic=overview-whats-new&utm_source=chatgpt.com)) |
| **Aras Innovator MCP 实践** | 将批准过的 PLM 业务动作暴露给 Agent，复用身份、访问策略、限流和审计 | **B+，R/有限 W**。目前更像官方 Labs 和参考架构，而不是所有客户都能直接启用的通用服务器。([community.aras.com](https://community.aras.com/blog/labs/work-smarter--safer-with-ai-connecting-aras-innovator-and-claude-mcp/16591)) |
| **Siemens Graph Studio** | 通过语义图和 SPARQL 向 Agent 暴露跨产品、工程和制造上下文 | **B+，R/V**。它体现了一个重要方向：Agent 不直接扫描所有文件，而是查询有语义关系的工程知识图谱。([blogs.sw.siemens.com](https://blogs.sw.siemens.com/rapidminer/ai-context-layer-knowledge-graph/?utm_source=chatgpt.com)) |
| **Siemens Xcelerator Developer Portal MCP** | 搜索 Siemens 产品、文档和 API | **B+，R**。适合构建工程知识和开发助手，不等于直接控制 Teamcenter、NX 或 Polarion。([developer.siemens.com](https://developer.siemens.com/index.html)) |
| **PTC ThingWorx MCP Server** | 将 ThingWorx 服务、资源和提示暴露给外部 Agent | **B，R/W**。目前属于 Public Preview，适合工业应用平台和设备上下文层。([community.ptc.com](https://community.ptc.com/iot-connectivity-tips-384/model-context-protocol-mcp-public-preview-in-thingworx-10-1-171187?utm_source=chatgpt.com)) |

本轮公开资料中，暂未确认到下面这些产品有公开、稳定、通用的官方 MCP：

- Siemens Polarion
- PTC Windchill
- Dassault 3DEXPERIENCE/CATIA Magic
- 主流 QMS 平台
- 多数传统 ERP/PLM 工程对象写入接口

它们一般都有 REST、SOAP、OSLC、SDK 或自动化 API，因此属于 **I 级：适合自建薄 MCP**，而不是“无法接入”。

---

# 四、嵌入式开发、元器件和硬件在环

## 1. 嵌入式开发与硬件在环

对于嵌入式固件、开发板和硬件在环测试，优先使用目标平台提供的编译器、调试服务器、仿真器和自动化 API，再通过受控 Broker 暴露给 Agent。适合建立下面这类有限工具：

```text
build_firmware
run_static_analysis
flash_candidate_image
reset_target
read_registers
collect_trace
run_hil_test
restore_golden_firmware
```

不要直接向主 Agent 暴露任意调试命令、任意寄存器写入或任意烧录文件路径。所有固件写入都应绑定目标标识、固件哈希、硬件版本、回滚镜像和失败后的安全状态。

## 2. 元器件、BOM 和供应链

现阶段更可靠的做法是使用供应商和分销商提供的正式 API 封装只读 MCP：

| 数据源 | 适合暴露给 Agent 的能力 |
|---|---|
| **DigiKey API** | 参数、库存、价格、报价和订单准备 |
| **Mouser API** | 实时价格、库存和器件搜索 |
| **Nexar/Octopart GraphQL** | 多供应商库存、价格、生命周期、交期和替代料 |
| **JLCPCB/LCSC** | 国内贴片库存和器件属性，但部分社区方案依赖非公开接口，应二次核验 |

DigiKey、Mouser 和 Nexar 都提供正式开发接口，因此生产环境应优先封装这些官方 API，而不是让 Agent 抓取网页。([developer.digikey.com](https://developer.digikey.com/))

建议只开放：

```text
search_part
compare_alternates
check_lifecycle
check_stock_snapshot
estimate_bom_cost
identify_single_source_risk
prepare_purchase_request
```

不开放 `place_order`，采购单和供应商变更必须走企业审批流。

---

# 五、测试测量是目前最有生产价值的方向之一

| 平台 | 能力 | 状态与评级 |
|---|---|---|
| **Keysight MCP Server for Instrument Control** | 自然语言生成测量流程，将请求转为经过验证的仪器命令，支持批准和执行记录 | **B+，V/X**。2026 年 7 月 31 日发布首个 Public Preview。架构中所有硬件指令都经过验证和批准管线，并保留完整会话日志。([keysight.com](https://www.keysight.com/us/en/lib/software-detail/computer-software/keysight-mcp-server-for-instrument-control.html?utm_source=chatgpt.com)) |
| **Keysight ADS MCP** | 查询和修改 ADS 设计、执行仿真、分析结果和辅助宏录制 | **A-/B+，R/V/W**。适合射频、微波和高速电路设计自动化。([helpfiles.keysight.com](https://helpfiles.keysight.com/kmsic/English/keysight_mcp_for_instrument_control/Content/overview.html)) |
| **NI Nigel for LabVIEW/TestStand** | 连接 MCP 工具、生成或辅助修改 VI 和测试序列 | **B+，主要是 MCP 客户端/Agent 能力**，不是一个通用的 LabVIEW MCP Server。([ni.com](https://www.ni.com/docs/en-US/bundle/labview/page/labview-changes.html?srsltid=AfmBOoraOKhkHncgql2vBnZyNuyrrdcs3wIXlje3xJJC9_Km5SkIm1s_)) |
| **IVI/VISA/SCPI/LXI** | 跨厂商仪器控制、资源发现、驱动和测量命令 | **I/A 级底层接口**。适合自建有限语义 MCP，是覆盖 Tektronix、Rohde & Schwarz、NI、Keysight 等多厂商设备的现实路径。([ivifoundation.org](https://ivifoundation.org/)) |

一个成熟测试 Agent 不应暴露一个通用的 `send_scpi(command)`，而应提供：

```text
identify_instrument
configure_scope_capture
configure_power_sweep
run_iv_curve
measure_noise_floor
run_protocol_compliance_subset
collect_waveforms
compare_against_limits
restore_safe_state
```

每个工具应内置：

- 仪器型号和能力检查
- 电压、电流、功率、频率上限
- 端口和通道映射
- DUT 保护条件
- 超时和急停
- 执行前预览
- 原始数据和配置快照

---

# 六、PLC、SCADA 和机器自动化

这一领域的官方支持比传统 PCB 软件快得多。

| 平台 | 能力 | 状态与评级 |
|---|---|---|
| **CODESYS Development System MCP Server** | 读取工程、创建和修改数据类型及 Structured Text POU、调用编译器、读取错误和库文档 | **A-/B+，R/V/W**。2026 年 4 月发布 1.0，7 月发布 1.1，并支持 SDK 客户添加自定义工具。([us.codesys.com](https://us.codesys.com/ecosystem/up-to-date/release-lifecycle/releases-updates/development-system-mcp-server/?utm_source=chatgpt.com)) |
| **Beckhoff TwinCAT CoAgent** | 结合 TwinCAT 工程、诊断和外部系统上下文，使用本地或云模型辅助自动化工程 | **B+，R/V/W**。自然语言设备控制和路径规划类功能必须单独视为高风险 X 能力。([beckhoff.com](https://www.beckhoff.com/en-us/products/automation/twincat-projects-with-ai-supported-engineering/)) |
| **Siemens WinCC OA MCP** | 向 Agent 提供 SCADA 数据、对象和运行上下文 | **B+，R/有限 W**。Siemens 还展示了 WinCC Unified PC Runtime 的 MCP 接入。([support.industry.siemens.com](https://support.industry.siemens.com/cs/document/109994210/wincc-oa-mcp-server?lc=en-ch)) |
| **Siemens Engineering Agent** | 连接 TIA Portal，辅助 PLC 工程和自动化任务 | **B+，R/V/W**。属于 Siemens 自有 Agent 体系，不一定是开放通用 MCP。([siemens.com](https://www.siemens.com/en-us/products/tia-portal/eigen-engineering-agent/?utm_source=chatgpt.com)) |
| **Ignition MCP Module** | 将 Ignition 的 OT 数据与外部 LLM 客户端连接 | **B-/C+**。截至 2026 年仍属于 Early Access、范围有限。([forum.inductiveautomation.com](https://forum.inductiveautomation.com/t/mcp-module-early-access/113966?utm_source=chatgpt.com)) |
| **N3uron、vNode MCP 模块** | 查询实时/历史数据、报警和 KPI | **B，R/V**。适合做工业数据接入层，不应绕开 PLC 安全逻辑直接控机。([n3uron.com](https://n3uron.com/iiot-platform-whats-new/)) |

对 Mitsubishi、Omron、Schneider 等 PLC，目前能找到一些社区桥接项目，但尚不应视为厂商支持的生产级 MCP。最现实的方案是：

```text
PLC 厂商工程 API
        ↓
OPC UA / 厂商驱动
        ↓
受控工业数据平台
        ↓
语义 MCP
        ↓
Agent
```

而不是：

```text
Agent → 任意 PLC 地址写入
```

---

# 七、制造现场和工业数据层

## 1. 当前最重要的官方工业 MCP

| 平台 | 能力 | 状态与评级 |
|---|---|---|
| **OPC Foundation OPC UA MCP Server** | 浏览节点、读取、写入、历史查询、方法调用、订阅、诊断，并提供 Robotics、Vision 等受限 Profile | **A，R/V；W/X 为 B**。这是目前最重要的跨厂商工业 MCP 基础，但生产环境必须配置证书信任、工具 Profile、地址范围和写权限。([github.com](https://github.com/OPCFoundation/UA-.NETStandard/blob/master/docs/McpServer.md)) |
| **OPC UA Companion Specifications for AI** | 将 430 多个行业 Companion Specifications 转为适合 RAG、MCP 和 AI 工程流程的格式 | **A，R**。有助于 Agent 理解设备语义，而不仅是读取 Tag 名。([opcfoundation.org](https://opcfoundation.org/news/press-releases/opc-foundation-advances-opc-ua-for-the-ai-era-with-companion-specifications-optimized-for-agentic-ai/?utm_source=chatgpt.com)) |
| **Tulip MCP** | 访问站点、机器、用户和表格，读取指标、更新记录、触发受控事件 | **A-/B+，R/W**。使用 Tulip API 权限治理，MCP 已公开提供，Agent 功能仍在扩展。([tulip.co](https://tulip.co/blog/introducing-tulip-mcp/?utm_source=chatgpt.com)) |
| **HighByte Intelligence Hub Industrial MCP** | 将工业数据管道作为 Agent 工具，查询连接系统中的实时和历史数据 | **A-，R/V**。HighByte 4.2 已将 Industrial MCP Server 作为 GA 功能发布。([highbyte.com](https://www.highbyte.com/news/press-releases/highbyte-releases-industrial-mcp-server-for-agentic-ai?utm_source=chatgpt.com)) |
| **Cognite Industrial MCP** | 在用户权限下访问工业知识图谱、资产、时序数据、文档和关系 | **B+，R/V**。2026 年仍为 Preview，但在复杂工业语义和 Agent 评估方面很有价值。([hub.cognite.com](https://hub.cognite.com/product-updates-494/q3-2026-product-release-democratizing-ai-and-1000x-scalability-6723)) |
| **AWS IoT SiteWise MCP** | 工业模型创建、结构验证、单位和数据类型检查、资产建模 | **B+，R/V/W**。AWS 已发布开源 SiteWise MCP，适合资产模型和工业数据上下文，不适合直接执行实时安全控制。([aws.amazon.com](https://aws.amazon.com/about-aws/whats-new/2025/09/aws-sitewise-mcp-server-modeling/)) |
| **PTC ThingWorx MCP** | 将工业应用中的服务、设备和业务逻辑暴露为 MCP 工具 | **B，R/W**。目前为 Public Preview。([community.ptc.com](https://community.ptc.com/iot-connectivity-tips-384/model-context-protocol-mcp-public-preview-in-thingworx-10-1-171187?utm_source=chatgpt.com)) |
| **Microsoft Fabric Real-Time Intelligence MCP** | 查询实时事件、时序数据和企业分析上下文 | **B+，R/V**。适合 IT/OT 汇聚后的分析层，而不是设备控制层。([learn.microsoft.com](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/)) |
| **SAP Integration Suite MCP** | 将企业业务流程和 API 接入 Agent | **A-/B+，R/W**。适合工单、物料、采购和生产业务层，不适合直接接设备。([help.sap.com](https://help.sap.com/docs/integration-suite/isuite-integrations-and-apis/model-context-protocol-mcp)) |

AVEVA 已宣布面向 Operations Control 等产品的 MCP 集成方向，但公开资料仍以计划、预览和后续发布为主。社区 PI System MCP 项目也明确不属于产品化官方支持，因此目前不应把 AVEVA PI 归类为成熟官方 MCP。([aveva.com](https://www.aveva.com/en/about/news/press-releases/2026/aveva-announces-new-capabilities-to-embed-ai-across-industrial-organizations-and-data-infrastructure-at-aveva-world-2026/?utm_source=chatgpt.com))

---

# 八、电子制造真正应优先接入的是行业协议

电子制造现场常常不需要针对每台贴片机、SPI、AOI、回流炉单独编写 MCP。更好的方法是在稳定协议之上建立语义层。

| 协议或数据格式 | 用途 | Agent 适合做什么 |
|---|---|---|
| **IPC-CFX / IPC-2591** | PCBA 生产设备和主机系统双向交换状态、生产、质量和维护信息 | 查询产线状态、设备报警、缺陷趋势、工单进度和消耗；通过审批后的业务命令触发有限动作。([ipc.org](https://www.ipc.org/ipc-2591-connected-factory-exchange-cfx)) |
| **IPC-HERMES-9852 / Hermes** | SMT 设备之间传递 PCB 流转、板卡标识和上下游状态 | 查询在制品位置、阻塞和换线状态；不应用作自由设备控制接口。([the-hermes-standard.info](https://www.the-hermes-standard.info/)) |
| **OPC UA** | PLC、SCADA、设备和工业应用的语义数据交换 | 跨设备读取、诊断、调用白名单方法；写入通过工业策略网关。([github.com](https://github.com/OPCFoundation/UA-.NETStandard/blob/master/docs/McpServer.md)) |
| **MTConnect** | CNC、机床和制造设备状态数据 | 查询运行、停机、负载、程序和故障上下文，适合只读分析。([mtconnect.org](https://www.mtconnect.org/documentation)) |
| **IVI/VISA/SCPI/LXI** | 测试测量设备 | 生成和执行受控测试步骤，采集原始数据。([ivifoundation.org](https://ivifoundation.org/)) |
| **IPC-2581** | PCB 设计与制造之间的双向完整数据交换 | 做发布包完整性、层叠、钻孔、材料、装配和可制造性检查。([ipc2581.com](https://www.ipc2581.com/)) |
| **ODB++** | PCB/封装制造产品模型 | 解析制造层、网络、钻孔和装配数据，做独立 CAM/DFM 审查。([odbplusplus.com](https://odbplusplus.com/)) |
| **Gerber X3** | PCB 图形和元件装配信息 | 渲染、层比对、极性检查、开窗检查、装配和发布包核验。([ucamco.com](https://www.ucamco.com/en/gerber/gerber-x3)) |

### 关键架构原则

不要直接把这些底层接口全部暴露给模型：

```text
write_opc_tag(address, value)
send_scpi(command)
execute_plc_code(text)
```

应转换成有限语义工具：

```text
acknowledge_alarm
request_recipe_change
start_approved_test
put_machine_in_maintenance_mode
retrieve_board_genealogy
compare_active_recipe
restore_validated_configuration
```

语义工具内部再调用 OPC UA、CFX 或 SCPI。

---

# 九、PCB 和电子设计社区生态重新评级

PCB 仍然重要，但不再是整个工程 Agent 的中心。

| 平台 | 当前可用方案 | 现实评级 |
|---|---|---|
| **KiCad** | 官方 IPC API、`kicad-cli`、Konnect、Universal Netlist、kicad-happy、其他社区 MCP | **底层接口 A；MCP B**。最适合搭建开放、可回滚、可由 Git 管理的 PCB Agent。([github.com](https://github.com/Finerestaurant/kicad-mcp-python)) |
| **Altium Designer** | eda-agent、Altium library MCP、只读 schematic MCP、Universal Netlist | **C+/B-**。适合读取、设计审查和器件库生成；不建议让社区 MCP 任意执行脚本或修改生产 PCB。([github.com](https://github.com/salitronic/eda-agent)) |
| **EasyEDA Pro** | VLab EasyEDA MCP、网表验证 Skills、其他社区扩展 | **B-/C+**。适合读取、追网、BOM、局部修改和打样前审查。([github.com](https://github.com/vlab-software/easyeda_mcp)) |
| **SPICE/ngspice/LTspice** | ngspice MCP、LTspice/SpiceLib 社区桥接 | **C+/B-**。仿真引擎本身成熟，MCP 包装较新；结果解析和模型可信性比启动仿真更重要。([github.com](https://github.com/gtnoble/ngspice-mcp)) |
| **FreeCAD、SolidWorks、Rhino** | 多个社区 MCP 或 COM/Python 桥接 | **C**。适合简单特征、格式转换、截图和几何查询；复杂装配和生产模型修改仍需原生 API、版本控制和人工审核。([github.com](https://github.com/sandraschi/freecad-mcp)) |

PCB 领域最适合先实现的不是“整板自动设计”，而是：

```text
datasheet_to_library_part
schematic_semantic_review
trace_power_tree
trace_signal_path
check_pin_electrical_types
validate_decoupling
compare_netlist_revisions
review_stackup_and_constraints
validate_gerber_odb_ipc2581
check_bom_cpl_consistency
prepare_release_evidence
```

这些任务比自主放置器件和自主布线更容易量化正确性。

---

# 十、目前明显不成熟或容易被误判的领域

本轮公开资料检索中，尚未确认以下领域存在公开、稳定、可直接配置的官方通用 MCP：

| 领域 | 当前更现实的接入方式 |
|---|---|
| **SolidWorks、CATIA、NX、Creo** | COM、宏、插件 SDK、CAA、NX Open、Creo Toolkit |
| **EPLAN、Zuken E3/CR-8000、Siemens Capital** | 厂商 API、数据导入导出、线束数据库或专用插件 |
| **Altium 完整工程写入** | 脚本 API、受控插件、独立只读审查 |
| **Polarion、Windchill、3DEXPERIENCE** | REST、OSLC、SDK、企业集成平台 |
| **ZEISS、Hexagon 等计量软件** | 原生自动化接口、质量数据库、PMI 和检测计划接口 |
| **主流 QMS/CAPA 系统** | REST API、工作流引擎、报表和文档检索 |
| **直接工业机器人运动控制** | ROS 2、厂商 SDK、PLC/机器人控制器、安全控制和数字孪生 |

例如，Zuken 已有完整的电气和 PCB 数字工程产品，ZEISS 有 PiWeb、CALYPSO 和 PMI 驱动的质量工作流，但本轮没有确认到它们提供公开的官方通用 MCP Server。([zuken.com](https://www.zuken.com/us/solution/digital-engineering/?utm_source=chatgpt.com))

这不意味着这些软件不适合 Agent，而是意味着应使用：

> **成熟原生 API → 企业策略层 → 小型语义 MCP**

而不是等待厂商提供一个万能 MCP。

---

# 十一、目前真正成熟、可直接借鉴的 Skills

MCP 只是“手”，Skills 才决定 Agent 是否按工程方法做事。

## 可直接复用或重点参考的现有 Skills

| Skills 来源 | 适用范围 | 评级 |
|---|---|---|
| **MATLAB Agentic Toolkit Skills** | MATLAB 代码规范、测试、调试、应用构建、代码生成和工具箱使用 | **A-**。官方维护，并与 MATLAB MCP Core Server 配套。([mathworks.com](https://www.mathworks.com/products/matlab-agentic-toolkit.html?utm_source=chatgpt.com)) |
| **Simulink Agentic Toolkit Skills** | 需求、建模、仿真、调试、测试和 Model-Based Design 方法 | **A-**。目前工程方法结构最完整的 Skill 集之一。([mathworks.com](https://www.mathworks.com/products/simulink-agentic-toolkit.html?utm_source=chatgpt.com)) |
| **NVIDIA Isaac Sim Skills** | 场景构建、仿真执行、USD 修改和机器人工作流 | **B+**。适合数字孪生和机器人仿真。([forums.developer.nvidia.com](https://forums.developer.nvidia.com/t/isaac-sim-6-0-general-availability/372621)) |
| **Cognite Agent Builder Skills** | 工业数据查询、设备上下文、工业知识图谱和评估 | **B+**。适合大型工业数据平台，但 Industrial MCP 仍处于 Preview。([hub.cognite.com](https://hub.cognite.com/product-updates-494/q3-2026-product-release-democratizing-ai-and-1000x-scalability-6723)) |
| **kicad-happy** | KiCad 数据解析、数据手册、EMC、SPICE、采购和制造审查 | **B**。属于社区 Skill，但方向正确，适合作为 PCB 工程 SOP 的基础。([github.com](https://github.com/aklofas/kicad-happy)) |
| **EasyEDA Pro Skills** | 网表验证、原理图修改、BOM、DRC/ERC 和导出 | **B-/C+**。适合受控局部修改，不适合完全自主设计。([github.com](https://github.com/oaslananka/easyeda-mcp-pro/blob/main/skills/easyeda-workflow/SKILL.md)) |

---

# 十二、建议你的 Agent 自建的工程 Skills 体系

不要为每个软件写一份互不兼容的提示词。应建立软件无关的工程 Skill，再把不同软件映射成工具。

## 1. 需求和系统工程

```text
requirements_ingest
requirement_quality_review
requirement_traceability
interface_contract_review
system_architecture_consistency
change_impact_analysis
eco_ecN_planning
verification_matrix_generation
```

输出应包括：

- 需求 ID 和版本
- 上下游追溯关系
- 未覆盖需求
- 冲突或模糊需求
- 影响的设计对象、测试和生产文件
- 建议变更，但不自动批准变更

## 2. 电子和 PCB

```text
datasheet_to_part
part_library_validation
schematic_semantic_review
power_tree_review
signal_path_review
clock_reset_review
si_pi_emc_review
pcb_dfm_dfa_dft_review
bom_risk_analysis
manufacturing_package_validation
```

其中 `datasheet_to_part` 不应只画一个符号，还应生成：

- 引脚表和来源页码
- 电气类型
- 未使用引脚处理
- 封装尺寸和公差
- Pin 1 方向
- 热焊盘和过孔要求
- 推荐 land pattern
- 3D 模型关联
- 数据手册与生成对象之间的核对报告

## 3. 固件和硬件联调

```text
firmware_build_and_static_check
board_bringup_plan
flash_and_restore
register_configuration_review
peripheral_smoke_test
fault_injection_plan
hardware_firmware_interface_check
trace_and_log_analysis
```

任何烧录操作必须绑定：

- 确认过的目标设备唯一标识
- 固件哈希值
- 支持的硬件版本
- 电源和启动模式
- 回滚固件
- 失败后的安全状态

## 4. 仿真与验证

```text
simulation_plan
model_configuration_review
parameter_sweep
sensitivity_analysis
results_sanity_check
model_measurement_correlation
regression_test_generation
evidence_package_generation
```

Agent 不能把“仿真成功运行”当作“设计正确”。Skill 必须检查：

- 单位
- 边界条件
- 网格或求解器收敛
- 模型来源
- 参数范围
- 与实测数据的偏差
- 不确定性和适用范围

## 5. 测试测量

```text
test_recipe_generation
instrument_capability_check
guarded_test_execution
limit_and_uncertainty_review
waveform_analysis
failure_reproduction
test_station_diagnosis
calibration_status_check
```

## 6. 制造和质量

```text
work_order_context
line_status_diagnosis
equipment_alarm_triage
recipe_comparison
board_genealogy_trace
defect_cluster_analysis
spc_anomaly_detection
first_pass_yield_analysis
ncr_triage
capa_evidence_collection
supplier_change_impact
release_readiness_review
```

---

# 十三、每个 Skill 都应包含的结构

一个成熟 Skill 不应只是 Markdown 中的几段建议。至少需要下面的契约：

```yaml
name: pcb_release_validation
version: 1.3.0

inputs:
  - design_revision
  - manufacturing_profile
  - approved_bom
  - approved_stackup

sources_of_truth:
  - plm_revision
  - eda_native_project
  - manufacturer_rules

preconditions:
  - project_is_clean
  - revision_is_locked
  - all_parts_are_resolved

allowed_tools:
  - inspect_design
  - run_native_drc
  - parse_gerber
  - parse_ipc2581
  - compare_bom_cpl

forbidden_tools:
  - arbitrary_shell
  - arbitrary_python
  - overwrite_source_project
  - submit_purchase_order

approval:
  required_before:
    - create_release_record
    - publish_manufacturing_package

validators:
  - native_erc
  - native_drc
  - independent_cam_check
  - checksum_manifest

rollback:
  - restore_source_revision
  - invalidate_release_candidate

evidence:
  - tool_call_log
  - source_revision
  - generated_files
  - validation_results
  - human_approver
```

这里最重要的是：

> Skill 必须定义**来源、前置条件、允许工具、禁止工具、验证器、审批点、回滚和证据**，而不仅是告诉模型“认真检查”。

---

# 十四、推荐的总体 Agent 架构

```text
工程师 / 制造人员
        │
        ▼
┌──────────────────────────────┐
│  Engineering Agent Orchestrator
│  任务拆分、上下文管理、计划生成
└──────────────┬───────────────┘
               │
        Skill Router
               │
┌──────────────┼─────────────────────────┐
│              │                         │
▼              ▼                         ▼
只读上下文层    工程验证沙盒               受控执行层
Requirements   MATLAB / Simulink         Change Broker
PLM / BOM      Ansys / SPICE             Test Broker
Docs / Specs   EDA native checks         OT Action Broker
Historian      Digital twin              Release Broker
│              │                         │
└──────────────┴────────────┬────────────┘
                           ▼
                  Policy / Approval Gateway
                           │
         ┌─────────────────┼────────────────┐
         ▼                 ▼                ▼
       EDA/CAD          仪器与测试站       PLC/MES/设备
      MCP/API            MCP/IVI          OPC UA/CFX
         │                 │                │
         └─────────────────┼────────────────┘
                           ▼
              Evidence Store / Git / PLM Audit
```

## 主 Agent 只需要十几个高层工具

```text
inspect_product_context
trace_requirement
query_design
trace_signal_or_asset
search_component
plan_changeset
apply_approved_changeset
run_native_validation
run_simulation_candidate
prepare_test_recipe
execute_guarded_test
query_factory_context
diagnose_alarm
validate_release_package
rollback_changeset
```

不要让主 Agent 同时看到几百个 CAD、PLC、仪器和 MES 原子命令。

---

# 十五、必须实施的安全和工程边界

MCP 自身解决的是连接和工具发现，并不自动保证工程正确性或工业安全。官方安全指南要求敏感和企业级场景使用明确授权、访问控制和用户同意；制造领域还必须处理可靠性、评估和安全边界。([modelcontextprotocol.io](https://modelcontextprotocol.io/docs/2026-07-28/tutorials/security/security_best_practices))

生产环境至少需要：

1. **默认只读。**
2. **先生成计划和 diff，再允许写入。**
3. **写入绑定源工程版本和哈希值。**
4. **所有修改必须可回滚。**
5. **MCP 不能代替原生编译器、ERC、DRC、仿真器和测试限值。**
6. **禁止任意 Shell、Python、Tcl、SKILL、SCPI 和 PLC 地址写入。**
7. **高风险工具只接受结构化、有限枚举参数。**
8. **物理设备必须有独立联锁、急停和安全控制器。**
9. **控制模型无权修改自己的权限和安全阈值。**
10. **每次工具调用记录操作者、模型、输入、输出、版本和审批人。**
11. **MCP Server 升级前必须跑 Golden Project 回归。**
12. **发布、下单、烧录量产固件、下发配方和启停设备必须人工批准。**

Keysight 的做法值得借鉴：所有指令先经过 MCP Server 的验证与批准管线，再到仪器，并保留完整会话日志。([helpfiles.keysight.com](https://helpfiles.keysight.com/kmsic/English/keysight_mcp_for_instrument_control/Content/architecture.html?utm_source=chatgpt.com))

OPC UA MCP 也应使用受限 Profile，只开放当前 Agent 任务需要的浏览、读取、诊断或方法调用，而不是直接启用完整工具集。([github.com](https://github.com/OPCFoundation/UA-.NETStandard/blob/master/docs/McpServer.md))

---

# 十六、按业务场景推荐的实际组合

## 场景 A：电子产品研发与 PCB

```text
Jama Connect 或 IBM ELM
+ DigiKey / Mouser / Nexar 薄 MCP
+ KiCad IPC / Altium 只读适配器
+ MATLAB / Simulink
+ Ansys / SPICE
+ Keysight 或 IVI/VISA 测试 Broker
+ Gerber / IPC-2581 / ODB++ 独立发布检查
```

优先自动化：

1. 数据手册和器件库核对
2. 需求到测试追溯
3. 电源树和接口审查
4. BOM 生命周期风险
5. 仿真与实测关联
6. 发布包一致性

最后才是自动改图和自动布线。

---

## 场景 B：SMT、EMS 和 PCBA 工厂

```text
IPC-CFX
+ Hermes
+ OPC UA MCP
+ Tulip / HighByte / Cognite / ThingWorx
+ AOI/SPI/ICT 测试数据
+ MES/ERP/PLM 薄 MCP
+ SPC/质量 Skill
```

优先自动化：

- 查询当前工单和设备状态
- 板卡 genealogy
- 缺陷聚类
- 抛料和停机原因分析
- AOI/SPI 与工艺参数关联
- 换线准备
- 维修建议
- NCR/CAPA 证据收集

不应最先自动化：

- 自动修改回流炉温区
- 自动下发贴片程序
- 自动放行不合格品
- 自动关闭质量事件

---

## 场景 C：PLC、设备制造商和自动化集成商

```text
CODESYS MCP
或 TwinCAT CoAgent / Siemens Engineering Agent
+ OPC UA MCP
+ MATLAB/Simulink 或 Isaac Sim
+ Git/版本库
+ 编译器和硬件在环测试
```

优先实现：

- 读取工程结构
- 生成候选 Structured Text
- 编译和静态检查
- I/O 映射核对
- 状态机一致性检查
- 报警和诊断说明
- 仿真测试生成
- 变更影响报告

PLC 程序下载到真实控制器应属于独立、审批后的部署阶段。

---

## 场景 D：硬件测试实验室

```text
Keysight MCP
+ NI LabVIEW/TestStand Agent
+ IVI/VISA/SCPI Broker
+ MATLAB
+ 校准数据库
+ 测试证据存储
```

优先实现：

- 仪器识别和能力核对
- 测试方案生成
- 测量参数预览
- 数据采集和统计
- 自动报告
- 失败复现
- 测试站健康诊断

---

# 十七、建议的落地优先级

## 第一阶段：只读数字线程

接入：

- 需求
- PLM/BOM
- 数据手册
- EDA 工程读取
- 历史测试数据
- MES/设备历史数据

目标是回答“这个设计为什么这样做”“这个缺陷影响哪些版本”“这块板经过了哪些工序”。

## 第二阶段：验证和仿真

接入：

- MATLAB/Simulink
- Ansys/SPICE
- 编译器
- ERC/DRC
- Gerber/IPC-2581/ODB++
- 测试数据分析

Agent 可以运行工具，但不能修改正式发布对象。

## 第三阶段：受控工程写入

开放：

- 创建候选分支
- 修改候选模型
- 生成测试代码
- 创建器件库候选
- 生成 PLC 候选代码
- 创建 ECO 草案

所有结果通过 diff、原生验证和人工审批。

## 第四阶段：物理执行

仅对经过风险分析的有限任务开放：

- 运行批准过的测试
- 切换到维护模式
- 执行安全范围内的仪器配置
- 调用 PLC/设备白名单方法
- 恢复已验证配置

MHS 值得持续关注，但截至当前仍是研究预览；近期生产方案仍应以 OPC UA、SCPI、ROS 2 和厂商控制器为底层，并由独立安全系统负责最后执行。([anthropic.com](https://www.anthropic.com/news/model-hardware-standard-research-preview?utm_source=chatgpt.com))

---

# 最终选型建议

对一个覆盖“工程研发 + 电子制造”的通用 Agent，不应寻找一个万能 MCP。更合理的基础组合是：

```text
需求与追溯：
Jama Connect 或 IBM Engineering AI Hub

建模与仿真：
MATLAB / Simulink Agentic Toolkit
+ Ansys PyAnsys MCP

CAD/CAM：
Autodesk Fusion MCP
+ 针对现有 MCAD 的原生 API 薄 MCP

EDA/PCB：
KiCad IPC / Konnect
或 Altium 只读审查 + 受控插件

嵌入式：
厂商 CLI/JTAG 受控 Broker

测试：
Keysight MCP
+ IVI/VISA/SCPI 语义 Broker
+ NI LabVIEW/TestStand

制造现场：
OPC UA MCP
+ IPC-CFX
+ Hermes
工业数据与 MES：
Tulip、HighByte、Cognite、ThingWorx 或 AWS SiteWise

企业业务：
PLM/MES/ERP 官方 API 薄 MCP

治理：
Skill Router
+ Policy Gateway
+ Human Approval
+ Native Validators
+ Evidence Store
+ Rollback
```

其中最值得优先投入的五个能力是：

1. **需求—设计—测试—制造的追溯**
2. **独立设计和发布验证**
3. **受控仪器测试**
4. **工业数据和设备诊断**
5. **带 diff、审批和回滚的工程修改**

这五项成熟后，再考虑自主布线、自动修改 PLC、自动调机或机器人闭环控制。工程 Agent 的可靠性主要来自**约束、验证、证据和安全架构**，而不是 MCP Server 数量。
