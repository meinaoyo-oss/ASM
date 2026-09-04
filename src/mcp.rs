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
        ReleaseReport, ReleaseRequest, Severity, Status, classify_pcb_file, compare_bom_cpl,
        inspect_package, parse_bom, parse_cpl, read_package_member, validate_bom,
        validate_gerber_set, validate_ipc2581, validate_release,
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
            "formats": ["directory", "zip", "bom-csv", "cpl-csv", "gerber-x2-x3", "excellon", "ipc-2581"],
            "profiles": ["generic", "jlcpcb"],
            "release_kinds": ["fabrication", "assembly"],
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
    version = "0.1.0",
    instructions = "只读检查 PCB 制造发布包。工具结果不是制造放行、法规认证或人工批准。"
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
