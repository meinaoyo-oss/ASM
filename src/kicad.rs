use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;
use tokio::{process::Command, time::timeout};

use crate::{
    config::AppConfig,
    domain::{CheckResult, Finding, Severity, Status},
};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadValidation {
    pub status: Status,
    pub version: String,
    pub checks: Vec<CheckResult>,
    pub reports: Vec<KicadReport>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadReport {
    pub kind: String,
    pub source: String,
    pub data: Value,
}

pub async fn run_kicad_checks(config: &AppConfig, project_path: &Path) -> Result<KicadValidation> {
    let cli = config
        .find_kicad_cli()
        .context("未找到 kicad-cli；请安装 KiCad 8+ 或在配置中设置 kicad.cli_path")?;
    let project_path = config.resolve_input(project_path)?;
    let inputs = find_inputs(&project_path)?;
    if inputs.schematic.is_none() && inputs.board.is_none() {
        return Err(anyhow!(
            "未在 {} 找到 .kicad_sch 或 .kicad_pcb",
            project_path.display()
        ));
    }

    let timeout_duration = Duration::from_secs(config.kicad.timeout_seconds);
    let version_output =
        run_command(&cli, &["version", "--format", "plain"], timeout_duration).await?;
    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_owned();
    let temporary = TempDir::new().context("无法创建 KiCad 检查临时目录")?;
    let mut checks = Vec::new();
    let mut reports = Vec::new();

    if let Some(schematic) = inputs.schematic {
        let output_path = temporary.path().join("erc.json");
        let report =
            run_design_check(&cli, "erc", &schematic, &output_path, timeout_duration).await?;
        checks.push(report.0);
        reports.push(report.1);
    }
    if let Some(board) = inputs.board {
        let output_path = temporary.path().join("drc.json");
        let report = run_design_check(&cli, "drc", &board, &output_path, timeout_duration).await?;
        checks.push(report.0);
        reports.push(report.1);
    }

    let status = checks.iter().fold(Status::Skipped, |status, check| {
        status.combine(check.status)
    });
    Ok(KicadValidation {
        status,
        version,
        checks,
        reports,
    })
}

struct KicadInputs {
    schematic: Option<PathBuf>,
    board: Option<PathBuf>,
}

fn find_inputs(path: &Path) -> Result<KicadInputs> {
    if path.is_dir() {
        let mut files = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        return Ok(KicadInputs {
            schematic: files
                .iter()
                .find(|path| has_extension(path, "kicad_sch"))
                .cloned(),
            board: files
                .iter()
                .find(|path| has_extension(path, "kicad_pcb"))
                .cloned(),
        });
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let stem = if extension == "kicad_pro" {
        Some(path.with_extension(""))
    } else {
        None
    };
    let schematic = if extension == "kicad_sch" {
        Some(path.to_owned())
    } else {
        stem.as_ref()
            .map(|base| base.with_extension("kicad_sch"))
            .filter(|candidate| candidate.is_file())
    };
    let board = if extension == "kicad_pcb" {
        Some(path.to_owned())
    } else {
        stem.as_ref()
            .map(|base| base.with_extension("kicad_pcb"))
            .filter(|candidate| candidate.is_file())
    };
    Ok(KicadInputs { schematic, board })
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

async fn run_design_check(
    cli: &Path,
    kind: &str,
    source: &Path,
    output_path: &Path,
    timeout_duration: Duration,
) -> Result<(CheckResult, KicadReport)> {
    let domain = if kind == "erc" { "sch" } else { "pcb" };
    let source_text = source.to_string_lossy().into_owned();
    let output_text = output_path.to_string_lossy().into_owned();
    let arguments = [
        domain,
        kind,
        "--format",
        "json",
        "--severity-all",
        "--output",
        output_text.as_str(),
        source_text.as_str(),
    ];
    let output = run_command(cli, &arguments, timeout_duration).await?;
    if !output.status.success() && !output_path.is_file() {
        return Err(anyhow!(
            "KiCad {} 执行失败: {}",
            kind.to_ascii_uppercase(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let bytes = fs::read(output_path)
        .with_context(|| format!("KiCad 未生成 {} 报告", kind.to_ascii_uppercase()))?;
    let data: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("KiCad {} 报告不是有效 JSON", kind.to_ascii_uppercase()))?;
    let violations = collect_violations(&data);
    let mut check = CheckResult::new(
        format!("kicad_{kind}"),
        format!(
            "KiCad {} 完成，发现 {} 项",
            kind.to_ascii_uppercase(),
            violations.len()
        ),
    );
    for violation in violations {
        let severity = violation
            .get("severity")
            .and_then(Value::as_str)
            .map(parse_severity)
            .unwrap_or(Severity::Warning);
        let message = violation
            .get("description")
            .or_else(|| violation.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("KiCad 报告了一项违规");
        let code = violation
            .get("type")
            .or_else(|| violation.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("VIOLATION");
        check.add(
            Finding::new(
                format!(
                    "KICAD_{}_{}",
                    kind.to_ascii_uppercase(),
                    normalize_code(code)
                ),
                severity,
                message,
            )
            .at_path(source.display().to_string()),
        );
    }
    Ok((
        check,
        KicadReport {
            kind: kind.to_owned(),
            source: source.display().to_string(),
            data,
        },
    ))
}

async fn run_command(cli: &Path, arguments: &[&str], timeout_duration: Duration) -> Result<Output> {
    let mut command = Command::new(cli);
    command.args(arguments).kill_on_drop(true);
    timeout(timeout_duration, command.output())
        .await
        .with_context(|| format!("kicad-cli 执行超过 {} 秒", timeout_duration.as_secs()))?
        .with_context(|| format!("无法启动 {}", cli.display()))
}

fn collect_violations(value: &Value) -> Vec<&serde_json::Map<String, Value>> {
    let mut output = Vec::new();
    collect_named_arrays(value, "violations", &mut output);
    output
}

fn collect_named_arrays<'a>(
    value: &'a Value,
    name: &str,
    output: &mut Vec<&'a serde_json::Map<String, Value>>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == name {
                    if let Value::Array(items) = child {
                        output.extend(items.iter().filter_map(Value::as_object));
                    }
                } else {
                    collect_named_arrays(child, name, output);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_named_arrays(item, name, output);
            }
        }
        _ => {}
    }
}

fn parse_severity(value: &str) -> Severity {
    match value.to_ascii_lowercase().as_str() {
        "error" | "critical" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        _ => Severity::Info,
    }
}

fn normalize_code(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::config::{FilesystemConfig, KicadConfig};

    #[tokio::test]
    async fn runs_fixed_kicad_checks_and_parses_findings() {
        let root = tempfile::tempdir().expect("temporary project");
        let schematic = root.path().join("board.kicad_sch");
        fs::write(&schematic, "(kicad_sch)").expect("schematic fixture");
        let fake_cli = root.path().join("kicad-cli");
        fs::write(
            &fake_cli,
            r#"#!/bin/sh
if [ "$1" = "version" ]; then
  printf '9.0.0\n'
  exit 0
fi
printf '{"violations":[{"severity":"warning","type":"pin_not_connected","description":"引脚未连接"}]}' > "$7"
"#,
        )
        .expect("fake cli");
        let mut permissions = fs::metadata(&fake_cli).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_cli, permissions).expect("executable fixture");

        let config = AppConfig {
            filesystem: FilesystemConfig {
                allowed_roots: vec![root.path().canonicalize().expect("canonical root")],
                ..FilesystemConfig::default()
            },
            kicad: KicadConfig {
                cli_path: Some(fake_cli),
                timeout_seconds: 2,
            },
        };
        let result = run_kicad_checks(&config, &schematic)
            .await
            .expect("KiCad validation");
        assert_eq!(result.version, "9.0.0");
        assert_eq!(result.status, Status::Warn);
        assert_eq!(
            result.checks[0].findings[0].code,
            "KICAD_ERC_PIN_NOT_CONNECTED"
        );
    }
}
