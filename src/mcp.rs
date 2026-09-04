use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    config::AppConfig,
    domain::{
        Artifact, CheckResult, Finding, ManufacturingProfile, PackageLimits, ReleaseKind,
        ReleaseReport, ReleaseRequest, Severity, Status, analyze_kicad_connectivity,
        analyze_requirement_impact, build_traceability_matrix, classify_pcb_file, compare_bom_cpl,
        compare_kicad_revisions, compare_kicad_schematic_pcb, inspect_kicad_project,
        inspect_package, parse_bom, parse_cpl, parse_kicad_document, parse_requirements,
        parse_trace_links, read_package_member, review_bom_risk, review_kicad_power_tree,
        review_requirement_quality, trace_kicad_signal, validate_bom, validate_gerber_set,
        validate_ipc2581, validate_release, validate_spice_netlist,
    },
    kicad::run_kicad_checks,
};

const SCHEMA_VERSION: &str = "1.0";
type PackageFiles = (Vec<Artifact>, Vec<(String, Vec<u8>)>);

#[derive(Clone)]
pub struct ElectronicsMcp {
    config: AppConfig,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ToolEnvelope {
    pub schema_version: String,
    pub status: Status,
    pub summary: String,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub artifacts: Vec<Artifact>,
    pub report_paths: Vec<String>,
    pub data: Value,
}

impl ToolEnvelope {
    fn success<T: Serialize>(
        summary: impl Into<String>,
        checks: Vec<CheckResult>,
        artifacts: Vec<Artifact>,
        data: &T,
    ) -> Self {
        let status = checks
            .iter()
            .fold(Status::Pass, |status, check| status.combine(check.status));
        let findings = checks
            .iter()
            .flat_map(|check| check.findings.iter().cloned())
            .collect();
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            status,
            summary: summary.into(),
            checks,
            findings,
            artifacts,
            report_paths: Vec::new(),
            data: serde_json::to_value(data).unwrap_or(Value::Null),
        }
    }

    fn failure(code: &str, error: impl std::fmt::Display) -> Self {
        let message = error.to_string();
        let finding = Finding::new(code, Severity::Error, message.clone());
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            status: Status::Fail,
            summary: message,
            checks: vec![CheckResult {
                id: "request".to_owned(),
                status: Status::Fail,
                summary: "请求未完成".to_owned(),
                findings: vec![finding.clone()],
            }],
            findings: vec![finding],
            artifacts: Vec::new(),
            report_paths: Vec::new(),
            data: Value::Null,
        }
    }

    fn from_release(report: &ReleaseReport) -> Self {
        Self {
            schema_version: report.schema_version.clone(),
            status: report.status,
            summary: report.summary.clone(),
            checks: report.checks.clone(),
            findings: report.findings.clone(),
            artifacts: report.artifacts.clone(),
            report_paths: report.report_paths.clone(),
            data: serde_json::to_value(report).unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectPackageParams {
    /// 发布包目录或 ZIP 文件路径。
    pub source: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateBomParams {
    /// BOM CSV 文件路径。
    pub path: String,
    /// 通用或 JLCPCB 字段规则。
    #[serde(default)]
    pub profile: ManufacturingProfile,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompareBomCplParams {
    /// BOM CSV 文件路径。
    pub bom_path: String,
    /// CPL/贴片坐标 CSV 文件路径。
    pub cpl_path: String,
    /// 通用或 JLCPCB 字段规则。
    #[serde(default)]
    pub profile: ManufacturingProfile,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateGerberParams {
    /// 包含 Gerber/钻孔文件的目录或 ZIP。
    pub source: String,
    /// 通用或 JLCPCB 文件集合规则。
    #[serde(default)]
    pub profile: ManufacturingProfile,
    /// 预期铜层数；省略时使用 profile 最小值。
    pub expected_copper_layers: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateIpc2581Params {
    /// IPC-2581 XML 文件路径。
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunKicadParams {
    /// KiCad 项目目录、.kicad_pro、.kicad_sch 或 .kicad_pcb 路径。
    pub project_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateReleaseParams {
    /// 发布包目录或 ZIP 文件路径。
    pub source: String,
    /// 通用或 JLCPCB 规则。
    #[serde(default)]
    pub profile: ManufacturingProfile,
    /// fabrication 只要求制造文件；assembly 还要求 BOM/CPL。
    #[serde(default)]
    pub release_kind: ReleaseKind,
    /// 预期铜层数；省略时使用 profile 最小值。
    pub expected_copper_layers: Option<u8>,
    /// 可选报告目录；只允许位于配置的 allowed_roots 内。
    pub report_directory: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RequirementsParams {
    /// JSON、CSV 或包含 REQ-* 标识的 Markdown 需求文件路径。
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraceabilityParams {
    /// 需求 JSON、CSV 或 Markdown 文件路径。
    pub requirements_path: String,
    /// 可选链接 JSON/CSV；省略时使用需求内声明的 targets。
    pub links_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RequirementImpactParams {
    /// 需求 JSON、CSV 或 Markdown 文件路径。
    pub requirements_path: String,
    /// 可选链接 JSON/CSV 文件路径。
    pub links_path: Option<String>,
    /// 要分析的需求 ID，例如 REQ-001。
    pub requirement_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BomRiskParams {
    /// BOM CSV 文件路径。
    pub path: String,
    /// 通用或 JLCPCB 字段规则。
    #[serde(default)]
    pub profile: ManufacturingProfile,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpiceParams {
    /// SPICE netlist 文件路径。
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KicadDesignParams {
    /// KiCad 项目目录、.kicad_pro、.kicad_sch 或 .kicad_pcb 路径。
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KicadSignalParams {
    /// KiCad 项目目录、.kicad_pro、.kicad_sch 或 .kicad_pcb 路径。
    pub path: String,
    /// 要查找的位号、网络名、标签、封装或库标识片段。
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KicadCompareParams {
    /// 左侧基线 KiCad 项目路径。
    pub left_path: String,
    /// 右侧候选 KiCad 项目路径。
    pub right_path: String,
}

#[tool_router]
impl ElectronicsMcp {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    #[tool(description = "返回 PCB 发布检查支持的格式、profile、安全边界和可选 KiCad CLI 状态。")]
    fn pcb_get_capabilities(&self) -> Json<ToolEnvelope> {
        let kicad_cli = self.config.find_kicad_cli();
        let data = json!({
            "server_version": env!("CARGO_PKG_VERSION"),
            "transport": "stdio",
            "formats": ["directory", "zip", "bom-csv", "cpl-csv", "gerber-x2-x3", "excellon", "ipc-2581", "requirements-json-csv-markdown", "trace-links-json-csv", "spice-netlist"],
            "profiles": ["generic", "jlcpcb"],
            "release_kinds": ["fabrication", "assembly"],
            "workflow_tools": ["requirements_ingest", "requirements_quality_review", "requirements_traceability", "requirements_change_impact", "bom_review_risk", "spice_validate_netlist"],
            "kicad_native_tools": ["kicad_inspect_design", "kicad_semantic_review", "kicad_power_tree_review", "kicad_trace_signal", "kicad_compare_revisions"],
            "kicad_connectivity_tools": ["kicad_connectivity_review"],
            "cross_domain_tools": ["kicad_schematic_pcb_consistency"],
            "network": false,
            "source_files_read_only": true,
            "allowed_roots": self.config.filesystem.allowed_roots,
            "kicad_cli": {
                "available": kicad_cli.is_some(),
                "path": kicad_cli
            }
        });
        Json(ToolEnvelope::success(
            "能力信息已返回",
            Vec::new(),
            Vec::new(),
            &data,
        ))
    }

    #[tool(description = "清点目录或 ZIP 中的发布文件，分类角色并计算 SHA-256；不修改或解压源包。")]
    fn pcb_inspect_package(
        &self,
        Parameters(params): Parameters<InspectPackageParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.inspect_package(&params.source) {
            Ok(inventory) => ToolEnvelope::success(
                format!("已清点 {} 个发布文件", inventory.files.len()),
                Vec::new(),
                inventory.files.clone(),
                &inventory,
            ),
            Err(error) => ToolEnvelope::failure("PACKAGE_INSPECTION_FAILED", error),
        })
    }

    #[tool(description = "验证 BOM CSV 的位号、值、封装、MPN、DNP 和重复项。")]
    fn pcb_validate_bom(
        &self,
        Parameters(params): Parameters<ValidateBomParams>,
    ) -> Json<ToolEnvelope> {
        Json(
            match self
                .read_input_file(&params.path)
                .and_then(|(path, bytes)| {
                    let document = parse_bom(&bytes, path.display().to_string())?;
                    let validation = validate_bom(&document, params.profile);
                    Ok((document, validation))
                }) {
                Ok((document, validation)) => ToolEnvelope::success(
                    format!("已验证 {} 个 BOM 位号", validation.reference_count),
                    vec![validation.check.clone()],
                    Vec::new(),
                    &json!({ "document": document, "validation": validation }),
                ),
                Err(error) => ToolEnvelope::failure("BOM_VALIDATION_FAILED", error),
            },
        )
    }

    #[tool(description = "比较 BOM 与 CPL 的有效位号、坐标、面别和旋转字段。")]
    fn pcb_compare_bom_cpl(
        &self,
        Parameters(params): Parameters<CompareBomCplParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.compare_bom_cpl(&params) {
            Ok((bom, cpl, comparison)) => ToolEnvelope::success(
                "BOM/CPL 一致性检查完成",
                vec![comparison.check.clone()],
                Vec::new(),
                &json!({ "bom": bom, "cpl": cpl, "comparison": comparison }),
            ),
            Err(error) => ToolEnvelope::failure("BOM_CPL_COMPARISON_FAILED", error),
        })
    }

    #[tool(description = "解析 Gerber X2/X3 与 Excellon 文件并验证铜层、板框和钻孔集合。")]
    fn pcb_validate_gerber_set(
        &self,
        Parameters(params): Parameters<ValidateGerberParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.load_package_files(&params.source) {
            Ok((artifacts, files)) => {
                let validation =
                    validate_gerber_set(&files, params.profile, params.expected_copper_layers);
                ToolEnvelope::success(
                    format!("已检查 {} 个 Gerber/钻孔文件", validation.files.len()),
                    vec![validation.check.clone()],
                    artifacts,
                    &validation,
                )
            }
            Err(error) => ToolEnvelope::failure("GERBER_VALIDATION_FAILED", error),
        })
    }

    #[tool(description = "解析 IPC-2581 XML，并返回版本、板层、器件和 BOM 条目摘要。")]
    fn pcb_validate_ipc2581(
        &self,
        Parameters(params): Parameters<ValidateIpc2581Params>,
    ) -> Json<ToolEnvelope> {
        Json(
            match self
                .read_input_file(&params.path)
                .and_then(|(path, bytes)| {
                    validate_ipc2581(&bytes, path.display().to_string())
                        .map_err(anyhow::Error::from)
                }) {
                Ok(validation) => ToolEnvelope::success(
                    "IPC-2581 检查完成",
                    vec![validation.check.clone()],
                    Vec::new(),
                    &validation,
                ),
                Err(error) => ToolEnvelope::failure("IPC2581_VALIDATION_FAILED", error),
            },
        )
    }

    #[tool(description = "使用配置好的 kicad-cli 对原生工程执行只读 ERC/DRC，报告在临时目录生成。")]
    async fn pcb_run_kicad_checks(
        &self,
        Parameters(params): Parameters<RunKicadParams>,
    ) -> Json<ToolEnvelope> {
        Json(
            match run_kicad_checks(&self.config, Path::new(&params.project_path)).await {
                Ok(validation) => ToolEnvelope::success(
                    format!("KiCad {} 检查完成", validation.version),
                    validation.checks.clone(),
                    Vec::new(),
                    &validation,
                ),
                Err(error) => ToolEnvelope::failure("KICAD_CHECK_FAILED", error),
            },
        )
    }

    #[tool(
        description = "执行完整 PCB fabrication/assembly 发布门禁，并可选生成 UTF-8 JSON/Markdown 证据报告。"
    )]
    fn pcb_validate_release(
        &self,
        Parameters(params): Parameters<ValidateReleaseParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.validate_release(&params) {
            Ok(report) => ToolEnvelope::from_release(&report),
            Err(error) => ToolEnvelope::failure("RELEASE_VALIDATION_FAILED", error),
        })
    }

    #[tool(
        description = "读取 JSON、CSV 或 Markdown 需求文件，返回稳定 ID、陈述、生命周期和声明目标。"
    )]
    fn requirements_ingest(
        &self,
        Parameters(params): Parameters<RequirementsParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_requirements(&params.path) {
            Ok(document) => {
                let quality = review_requirement_quality(&document);
                ToolEnvelope::success(
                    format!("已读取 {} 条需求", document.requirements.len()),
                    vec![quality.check.clone()],
                    Vec::new(),
                    &json!({ "document": document, "quality": quality }),
                )
            }
            Err(error) => ToolEnvelope::failure("REQUIREMENTS_INGEST_FAILED", error),
        })
    }

    #[tool(description = "审查需求质量，检查 ID 重复、空陈述、验证方法、状态和可复现来源。")]
    fn requirements_quality_review(
        &self,
        Parameters(params): Parameters<RequirementsParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_requirements(&params.path) {
            Ok(document) => {
                let quality = review_requirement_quality(&document);
                ToolEnvelope::success(
                    format!("需求质量审查完成：{} 条需求", quality.requirement_count),
                    vec![quality.check.clone()],
                    Vec::new(),
                    &quality,
                )
            }
            Err(error) => ToolEnvelope::failure("REQUIREMENTS_QUALITY_FAILED", error),
        })
    }

    #[tool(description = "建立需求到设计、测试、制造或其他对象的追溯矩阵，并报告未覆盖需求。")]
    fn requirements_traceability(
        &self,
        Parameters(params): Parameters<TraceabilityParams>,
    ) -> Json<ToolEnvelope> {
        Json(
            match self
                .read_traceability_inputs(&params.requirements_path, params.links_path.as_deref())
            {
                Ok((requirements, links)) => {
                    let quality = review_requirement_quality(&requirements);
                    let matrix = build_traceability_matrix(&requirements, links.as_ref());
                    ToolEnvelope::success(
                        format!(
                            "追溯矩阵完成：{} 条需求、{} 条链接",
                            matrix.requirement_count, matrix.link_count
                        ),
                        vec![quality.check.clone(), matrix.check.clone()],
                        Vec::new(),
                        &json!({ "requirements": requirements, "links": links, "matrix": matrix }),
                    )
                }
                Err(error) => ToolEnvelope::failure("REQUIREMENTS_TRACEABILITY_FAILED", error),
            },
        )
    }

    #[tool(description = "分析单条需求的设计/测试/制造影响目标及同标签相关需求。")]
    fn requirements_change_impact(
        &self,
        Parameters(params): Parameters<RequirementImpactParams>,
    ) -> Json<ToolEnvelope> {
        Json(
            match self
                .read_traceability_inputs(&params.requirements_path, params.links_path.as_deref())
            {
                Ok((requirements, links)) => match analyze_requirement_impact(
                    &requirements,
                    links.as_ref(),
                    &params.requirement_id,
                ) {
                    Ok(impact) => ToolEnvelope::success(
                        format!("已分析 {} 的变更影响", impact.requirement.id),
                        vec![impact.check.clone()],
                        Vec::new(),
                        &impact,
                    ),
                    Err(error) => ToolEnvelope::failure("REQUIREMENT_IMPACT_FAILED", error),
                },
                Err(error) => ToolEnvelope::failure("REQUIREMENT_IMPACT_INPUT_FAILED", error),
            },
        )
    }

    #[tool(
        description = "审查 BOM 生命周期、制造商、供应来源、替代料和缺失证据风险；不查询实时供应商 API。"
    )]
    fn bom_review_risk(&self, Parameters(params): Parameters<BomRiskParams>) -> Json<ToolEnvelope> {
        Json(
            match self
                .read_input_file(&params.path)
                .and_then(|(path, bytes)| {
                    let document = parse_bom(&bytes, path.display().to_string())?;
                    Ok(review_bom_risk(&document, params.profile))
                }) {
                Ok(report) => ToolEnvelope::success(
                    format!("BOM 风险审查完成：{} 项风险", report.risk_count),
                    vec![report.check.clone()],
                    Vec::new(),
                    &report,
                ),
                Err(error) => ToolEnvelope::failure("BOM_RISK_REVIEW_FAILED", error),
            },
        )
    }

    #[tool(
        description = "静态检查 SPICE 网表的元件、节点、地、分析指令、模型引用和终止符，不启动仿真器。"
    )]
    fn spice_validate_netlist(
        &self,
        Parameters(params): Parameters<SpiceParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_input_file(&params.path) {
            Ok((path, bytes)) => {
                let validation = validate_spice_netlist(&bytes, path.display().to_string());
                ToolEnvelope::success(
                    format!("SPICE 网表检查完成：{} 个元件", validation.component_count),
                    vec![validation.check.clone()],
                    Vec::new(),
                    &validation,
                )
            }
            Err(error) => ToolEnvelope::failure("SPICE_NETLIST_INPUT_FAILED", error),
        })
    }

    #[tool(
        description = "直接解析 KiCad 原生 S-expression，返回版本、元件、网络、标签和 PCB 层摘要。"
    )]
    fn kicad_inspect_design(
        &self,
        Parameters(params): Parameters<KicadDesignParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => ToolEnvelope::success(
                format!("已解析 KiCad 项目：{} 个元件", project.component_count),
                vec![project.check.clone()],
                Vec::new(),
                &project,
            ),
            Err(error) => ToolEnvelope::failure("KICAD_NATIVE_INSPECTION_FAILED", error),
        })
    }

    #[tool(description = "执行 KiCad 原生设计语义审查，检查位号、值、封装、连接证据和电源命名。")]
    fn kicad_semantic_review(
        &self,
        Parameters(params): Parameters<KicadDesignParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => {
                let power = review_kicad_power_tree(&project);
                ToolEnvelope::success(
                    "KiCad 原生语义审查完成",
                    vec![project.check.clone(), power.check.clone()],
                    Vec::new(),
                    &json!({ "project": project, "power_tree": power }),
                )
            }
            Err(error) => ToolEnvelope::failure("KICAD_SEMANTIC_REVIEW_FAILED", error),
        })
    }

    #[tool(
        description = "基于原理图 wire/junction/label/no_connect/pin 坐标推导网络，报告浮空端点、单引脚网和驱动缺口。"
    )]
    fn kicad_connectivity_review(
        &self,
        Parameters(params): Parameters<KicadDesignParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => {
                let mut checks = vec![project.check.clone()];
                let mut connectivity = Vec::new();
                for document in &project.documents {
                    let result = analyze_kicad_connectivity(document);
                    checks.push(result.check.clone());
                    connectivity.push(result);
                }
                ToolEnvelope::success(
                    "KiCad 原理图连通性审查完成",
                    checks,
                    Vec::new(),
                    &json!({ "project": project, "connectivity": connectivity }),
                )
            }
            Err(error) => ToolEnvelope::failure("KICAD_CONNECTIVITY_REVIEW_FAILED", error),
        })
    }

    #[tool(
        description = "交叉核对 KiCad 原理图与 PCB 的元件集合、封装、pin/pad 编号及带标签网络。"
    )]
    fn kicad_schematic_pcb_consistency(
        &self,
        Parameters(params): Parameters<KicadDesignParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => {
                let consistency = compare_kicad_schematic_pcb(&project);
                ToolEnvelope::success(
                    "KiCad 原理图与 PCB 一致性检查完成",
                    vec![project.check.clone(), consistency.check.clone()],
                    Vec::new(),
                    &json!({ "project": project, "consistency": consistency }),
                )
            }
            Err(error) => ToolEnvelope::failure("KICAD_SCHEMATIC_PCB_CHECK_FAILED", error),
        })
    }

    #[tool(description = "从 KiCad 原生工程中识别电源/地网络、疑似稳压器件及命名缺口。")]
    fn kicad_power_tree_review(
        &self,
        Parameters(params): Parameters<KicadDesignParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => {
                let power = review_kicad_power_tree(&project);
                ToolEnvelope::success(
                    format!("电源树审查完成：{} 个电源网络", power.power_nets.len()),
                    vec![power.check.clone()],
                    Vec::new(),
                    &power,
                )
            }
            Err(error) => ToolEnvelope::failure("KICAD_POWER_REVIEW_FAILED", error),
        })
    }

    #[tool(description = "按位号、网络名、标签、封装或库标识片段追踪 KiCad 原生设计对象。")]
    fn kicad_trace_signal(
        &self,
        Parameters(params): Parameters<KicadSignalParams>,
    ) -> Json<ToolEnvelope> {
        Json(match self.read_kicad_project(&params.path) {
            Ok(project) => {
                let trace = trace_kicad_signal(&project, &params.query);
                ToolEnvelope::success(
                    format!("信号查询完成：{}", params.query),
                    vec![trace.check.clone()],
                    Vec::new(),
                    &trace,
                )
            }
            Err(error) => ToolEnvelope::failure("KICAD_SIGNAL_TRACE_FAILED", error),
        })
    }

    #[tool(description = "比较两个 KiCad 原生工程版本，返回新增、删除和值/封装变化的元件及网络。")]
    fn kicad_compare_revisions(
        &self,
        Parameters(params): Parameters<KicadCompareParams>,
    ) -> Json<ToolEnvelope> {
        Json(
            match self.read_kicad_project(&params.left_path).and_then(|left| {
                self.read_kicad_project(&params.right_path)
                    .map(|right| (left, right))
            }) {
                Ok((left, right)) => {
                    let diff = compare_kicad_revisions(&left, &right);
                    ToolEnvelope::success(
                        "KiCad 版本差异分析完成",
                        vec![diff.check.clone()],
                        Vec::new(),
                        &diff,
                    )
                }
                Err(error) => ToolEnvelope::failure("KICAD_REVISION_COMPARE_FAILED", error),
            },
        )
    }

    fn package_limits(&self) -> PackageLimits {
        PackageLimits {
            max_entries: self.config.filesystem.max_archive_entries,
            max_file_bytes: self.config.filesystem.max_file_bytes,
            max_total_bytes: self.config.filesystem.max_archive_uncompressed_bytes,
            ..PackageLimits::default()
        }
    }

    fn inspect_package(&self, source: &str) -> Result<crate::domain::PackageInventory> {
        let source = self.config.resolve_input(Path::new(source))?;
        inspect_package(source, self.package_limits()).map_err(anyhow::Error::from)
    }

    fn read_input_file(&self, path: &str) -> Result<(PathBuf, Vec<u8>)> {
        let path = self.config.resolve_input(Path::new(path))?;
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err(anyhow!("输入不是普通文件: {}", path.display()));
        }
        if metadata.len() > self.config.filesystem.max_file_bytes {
            return Err(anyhow!(
                "输入文件超过 {} 字节限制: {}",
                self.config.filesystem.max_file_bytes,
                path.display()
            ));
        }
        Ok((path.clone(), fs::read(path)?))
    }

    fn compare_bom_cpl(
        &self,
        params: &CompareBomCplParams,
    ) -> Result<(
        crate::domain::BomDocument,
        crate::domain::CplDocument,
        crate::domain::BomCplComparison,
    )> {
        let (bom_path, bom_bytes) = self.read_input_file(&params.bom_path)?;
        let (cpl_path, cpl_bytes) = self.read_input_file(&params.cpl_path)?;
        let bom = parse_bom(&bom_bytes, bom_path.display().to_string())?;
        let cpl = parse_cpl(&cpl_bytes, cpl_path.display().to_string())?;
        let comparison = compare_bom_cpl(&bom, &cpl, params.profile);
        Ok((bom, cpl, comparison))
    }

    fn read_requirements(&self, path: &str) -> Result<crate::domain::RequirementDocument> {
        let (path, bytes) = self.read_input_file(path)?;
        parse_requirements(&bytes, path.display().to_string()).map_err(anyhow::Error::from)
    }

    fn read_traceability_inputs(
        &self,
        requirements_path: &str,
        links_path: Option<&str>,
    ) -> Result<(
        crate::domain::RequirementDocument,
        Option<crate::domain::TraceLinkDocument>,
    )> {
        let requirements = self.read_requirements(requirements_path)?;
        let links = links_path
            .map(|path| {
                let (path, bytes) = self.read_input_file(path)?;
                parse_trace_links(&bytes, path.display().to_string()).map_err(anyhow::Error::from)
            })
            .transpose()?;
        Ok((requirements, links))
    }

    fn read_kicad_project(&self, path: &str) -> Result<crate::domain::KicadProjectSnapshot> {
        let input = self.config.resolve_input(Path::new(path))?;
        let mut candidates = Vec::new();
        if input.is_dir() {
            for entry in fs::read_dir(&input)? {
                let entry = entry?;
                let candidate = entry.path();
                if candidate.is_file() && is_kicad_document_path(&candidate) {
                    candidates.push(candidate);
                }
            }
            candidates.sort();
        } else if is_kicad_document_path(&input) {
            candidates.push(input.clone());
        } else if has_extension(&input, "kicad_pro") {
            for extension in ["kicad_sch", "kicad_pcb"] {
                let candidate = input.with_extension(extension);
                if candidate.is_file() {
                    candidates.push(candidate);
                }
            }
        } else {
            return Err(anyhow!(
                "不是 KiCad 工程或原生设计文件: {}",
                input.display()
            ));
        }
        let mut documents = Vec::new();
        for candidate in candidates {
            let (path, bytes) = self.read_input_file(&candidate.to_string_lossy())?;
            documents.push(parse_kicad_document(&bytes, path.display().to_string())?);
        }
        Ok(inspect_kicad_project(
            documents,
            input.display().to_string(),
        ))
    }

    fn load_package_files(&self, source: &str) -> Result<PackageFiles> {
        let source = self.config.resolve_input(Path::new(source))?;
        let limits = self.package_limits();
        let inventory = inspect_package(&source, limits)?;
        let mut selected = Vec::new();
        for artifact in &inventory.files {
            if classify_pcb_file(&artifact.path).as_str() != "unknown" {
                selected.push((
                    artifact.path.clone(),
                    read_package_member(&source, &artifact.path, limits)?,
                ));
            }
        }
        Ok((inventory.files, selected))
    }

    fn validate_release(&self, params: &ValidateReleaseParams) -> Result<ReleaseReport> {
        let source = self.config.resolve_input(Path::new(&params.source))?;
        let mut report = validate_release(
            &source,
            ReleaseRequest {
                profile: params.profile,
                release_kind: params.release_kind,
                expected_copper_layers: params.expected_copper_layers,
                package_limits: self.package_limits(),
            },
        )?;
        report.summary = format!(
            "发布检查完成：{} 项检查，{} 个发现项，状态 {}",
            report.checks.len(),
            report.findings.len(),
            status_label(report.status)
        );
        if let Some(directory) = &params.report_directory {
            let directory = self.config.resolve_output_directory(Path::new(directory))?;
            report.report_paths = write_report_files(&directory, &report)?;
        }
        Ok(report)
    }
}

#[tool_handler(
    name = "electronics-manufacturing-mcp",
    version = "0.2.0",
    instructions = "只读分析工程电子制造文件、需求追溯、BOM 风险、SPICE 网表和 PCB 制造发布包。工具结果不是制造放行、法规认证或人工批准。"
)]
impl ServerHandler for ElectronicsMcp {}

pub async fn serve(config: AppConfig) -> Result<()> {
    let service = ElectronicsMcp::new(config).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn write_report_files(directory: &Path, report: &ReleaseReport) -> Result<Vec<String>> {
    fs::create_dir_all(directory)
        .with_context(|| format!("无法创建报告目录: {}", directory.display()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("系统时间早于 Unix epoch")?
        .as_millis();
    let json_path = directory.join(format!("pcb-release-{timestamp}.json"));
    let markdown_path = directory.join(format!("pcb-release-{timestamp}.md"));
    let paths = vec![
        json_path.display().to_string(),
        markdown_path.display().to_string(),
    ];
    let mut report_with_paths = report.clone();
    report_with_paths.report_paths = paths.clone();
    let mut json_bytes = serde_json::to_vec_pretty(&report_with_paths)?;
    json_bytes.push(b'\n');
    write_atomic(&json_path, &json_bytes)?;
    write_atomic(
        &markdown_path,
        render_markdown(&report_with_paths).as_bytes(),
    )?;
    Ok(paths)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("报告路径没有父目录")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.flush()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| anyhow!("无法写入报告 {}: {}", path.display(), error.error))?;
    Ok(())
}

fn render_markdown(report: &ReleaseReport) -> String {
    let mut text = format!(
        "# PCB 发布检查报告\n\n- 状态：{}\n- Profile：{:?}\n- 发布类型：{:?}\n- 制品数：{}\n- 发现项：{}\n\n## 检查\n\n",
        status_label(report.status),
        report.profile,
        report.release_kind,
        report.artifacts.len(),
        report.findings.len()
    );
    for check in &report.checks {
        text.push_str(&format!(
            "- `[{:?}]` `{}`：{}\n",
            check.status, check.id, check.summary
        ));
    }
    text.push_str("\n## 发现项\n\n");
    if report.findings.is_empty() {
        text.push_str("未发现规则问题。\n");
    } else {
        for finding in &report.findings {
            let path = finding
                .path
                .as_deref()
                .map(|value| format!(" ({value})"))
                .unwrap_or_default();
            text.push_str(&format!(
                "- `[{:?}]` `{}`：{}{}\n",
                finding.severity, finding.code, finding.message, path
            ));
        }
    }
    text
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Pass => "pass",
        Status::Warn => "warn",
        Status::Fail => "fail",
        Status::Skipped => "skipped",
    }
}

fn is_kicad_document_path(path: &Path) -> bool {
    has_extension(path, "kicad_sch") || has_extension(path, "kicad_pcb")
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}
