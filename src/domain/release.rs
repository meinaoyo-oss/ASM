use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::bom::{BomDocument, CplDocument, compare_bom_cpl, parse_bom, parse_cpl, validate_bom};
use super::gerber::{PcbFileRole, classify_pcb_file, validate_gerber_set};
use super::ipc2581::validate_ipc2581;
use super::package::{PackageLimits, inspect_package, read_package_member};
use super::profile::ManufacturingProfile;
use super::types::{Artifact, CheckResult, DomainResult, Finding, Severity, Status};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseKind {
    #[default]
    Fabrication,
    Assembly,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ReleaseRequest {
    #[serde(default)]
    pub profile: ManufacturingProfile,
    #[serde(default)]
    pub release_kind: ReleaseKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_copper_layers: Option<u8>,
    #[serde(default)]
    pub package_limits: PackageLimits,
}

impl Default for ReleaseRequest {
    fn default() -> Self {
        Self {
            profile: ManufacturingProfile::Generic,
            release_kind: ReleaseKind::Fabrication,
            expected_copper_layers: None,
            package_limits: PackageLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ReleaseReport {
    pub schema_version: String,
    pub status: Status,
    pub summary: String,
    pub profile: ManufacturingProfile,
    pub release_kind: ReleaseKind,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_paths: Vec<String>,
}

pub fn validate_release(
    source: impl AsRef<Path>,
    request: ReleaseRequest,
) -> DomainResult<ReleaseReport> {
    let source = source.as_ref();
    let inventory = inspect_package(source, request.package_limits)?;
    let package_kind = inventory.kind;
    let mut artifacts = inventory.files;
    for artifact in &mut artifacts {
        artifact.role = classify_pcb_file(&artifact.path).as_str().to_owned();
    }

    let mut checks = Vec::new();
    checks.push(CheckResult::new(
        "package",
        format!(
            "inventoried {} {} files",
            artifacts.len(),
            package_kind_name(package_kind)
        ),
    ));

    let gerber_inputs = read_gerber_inputs(source, &artifacts, request.package_limits, &mut checks);
    let gerber = validate_gerber_set(
        &gerber_inputs,
        request.profile,
        request.expected_copper_layers,
    );
    apply_gerber_statuses(&mut artifacts, &gerber);
    checks.push(gerber.check);

    let bom = load_bom(source, &artifacts, request.package_limits, &mut checks);
    let cpl = load_cpl(source, &artifacts, request.package_limits, &mut checks);
    apply_csv_statuses(&mut artifacts, &bom, &cpl);

    if let Some(document) = &bom {
        checks.push(validate_bom(document, request.profile).check);
    } else {
        let mut check = CheckResult::skipped("bom", "no BOM file was selected");
        if request.release_kind == ReleaseKind::Assembly {
            check.status = Status::Fail;
            check.add(Finding::new(
                "RELEASE_MISSING_BOM",
                Severity::Error,
                "assembly release requires a BOM CSV",
            ));
        }
        checks.push(check);
    }

    match (&bom, &cpl) {
        (Some(bom), Some(cpl)) => checks.push(compare_bom_cpl(bom, cpl, request.profile).check),
        _ => {
            let mut check =
                CheckResult::skipped("bom_cpl", "BOM/CPL comparison needs both CSV files");
            if request.release_kind == ReleaseKind::Assembly {
                check.status = Status::Fail;
                check.add(Finding::new(
                    "RELEASE_MISSING_CPL",
                    Severity::Error,
                    "assembly release requires a CPL CSV and BOM/CPL comparison",
                ));
            }
            checks.push(check);
        }
    }

    validate_ipc_files(source, &artifacts, request.package_limits, &mut checks);

    let status = checks
        .iter()
        .fold(Status::Pass, |status, check| status.combine(check.status));
    let findings = checks
        .iter()
        .flat_map(|check| check.findings.iter().cloned())
        .collect::<Vec<_>>();
    Ok(ReleaseReport {
        schema_version: "1.0".to_owned(),
        status,
        summary: format!(
            "{} {} release validation {} with {} findings",
            profile_name(request.profile),
            release_kind_name(request.release_kind),
            status_name(status),
            findings.len()
        ),
        profile: request.profile,
        release_kind: request.release_kind,
        checks,
        findings,
        artifacts,
        report_paths: Vec::new(),
    })
}

fn read_gerber_inputs(
    source: &Path,
    artifacts: &[Artifact],
    limits: PackageLimits,
    checks: &mut Vec<CheckResult>,
) -> Vec<(String, Vec<u8>)> {
    let mut inputs = Vec::new();
    for artifact in artifacts {
        let role = classify_pcb_file(&artifact.path);
        if !is_gerber_or_drill(&role) {
            continue;
        }
        match read_package_member(source, &artifact.path, limits) {
            Ok(bytes) => inputs.push((artifact.path.clone(), bytes)),
            Err(error) => {
                let mut check = CheckResult::new("package_member", "read package member");
                check.add(
                    Finding::new(
                        "PACKAGE_MEMBER_READ_FAILED",
                        Severity::Error,
                        error.to_string(),
                    )
                    .at_path(artifact.path.clone()),
                );
                checks.push(check);
            }
        }
    }
    inputs
}

fn load_bom(
    source: &Path,
    artifacts: &[Artifact],
    limits: PackageLimits,
    checks: &mut Vec<CheckResult>,
) -> Option<BomDocument> {
    let artifact = select_csv(artifacts, PcbFileRole::Bom)?;
    let bytes = match read_package_member(source, &artifact.path, limits) {
        Ok(bytes) => bytes,
        Err(error) => {
            add_parse_error(checks, "bom", &artifact.path, error.to_string());
            return None;
        }
    };
    match parse_bom(&bytes, artifact.path.clone()) {
        Ok(document) => Some(document),
        Err(error) => {
            add_parse_error(checks, "bom", &artifact.path, error.to_string());
            None
        }
    }
}

fn load_cpl(
    source: &Path,
    artifacts: &[Artifact],
    limits: PackageLimits,
    checks: &mut Vec<CheckResult>,
) -> Option<CplDocument> {
    let artifact = select_csv(artifacts, PcbFileRole::Cpl)?;
    let bytes = match read_package_member(source, &artifact.path, limits) {
        Ok(bytes) => bytes,
        Err(error) => {
            add_parse_error(checks, "cpl", &artifact.path, error.to_string());
            return None;
        }
    };
    match parse_cpl(&bytes, artifact.path.clone()) {
        Ok(document) => Some(document),
        Err(error) => {
            add_parse_error(checks, "cpl", &artifact.path, error.to_string());
            None
        }
    }
}

fn validate_ipc_files(
    source: &Path,
    artifacts: &[Artifact],
    limits: PackageLimits,
    checks: &mut Vec<CheckResult>,
) {
    let ipc_files = artifacts
        .iter()
        .filter(|artifact| classify_pcb_file(&artifact.path) == PcbFileRole::Ipc2581)
        .collect::<Vec<_>>();
    if ipc_files.is_empty() {
        checks.push(CheckResult::skipped(
            "ipc2581",
            "no IPC-2581 file was found",
        ));
        return;
    }
    for artifact in ipc_files {
        match read_package_member(source, &artifact.path, limits)
            .and_then(|bytes| validate_ipc2581(&bytes, artifact.path.clone()))
        {
            Ok(validation) => checks.push(validation.check),
            Err(error) => add_parse_error(checks, "ipc2581", &artifact.path, error.to_string()),
        }
    }
}

fn select_csv(artifacts: &[Artifact], requested: PcbFileRole) -> Option<&Artifact> {
    artifacts
        .iter()
        .find(|artifact| classify_pcb_file(&artifact.path) == requested)
}

fn add_parse_error(checks: &mut Vec<CheckResult>, id: &str, path: &str, message: String) {
    let mut check = CheckResult::new(id, "parse release CSV/XML data");
    check.add(Finding::new("RELEASE_PARSE_FAILED", Severity::Error, message).at_path(path));
    checks.push(check);
}

fn is_gerber_or_drill(role: &PcbFileRole) -> bool {
    matches!(
        role,
        PcbFileRole::GerberCopperTop
            | PcbFileRole::GerberCopperBottom
            | PcbFileRole::GerberCopperInner
            | PcbFileRole::GerberSolderMaskTop
            | PcbFileRole::GerberSolderMaskBottom
            | PcbFileRole::GerberSilkscreenTop
            | PcbFileRole::GerberSilkscreenBottom
            | PcbFileRole::BoardOutline
            | PcbFileRole::Drill
    )
}

fn apply_gerber_statuses(artifacts: &mut [Artifact], validation: &super::gerber::GerberValidation) {
    for artifact in artifacts {
        if let Some(file) = validation
            .files
            .iter()
            .find(|file| file.path == artifact.path)
        {
            artifact.parser_status = Some(file.status);
        }
    }
}

fn apply_csv_statuses(
    artifacts: &mut [Artifact],
    bom: &Option<BomDocument>,
    cpl: &Option<CplDocument>,
) {
    for artifact in artifacts {
        if bom
            .as_ref()
            .is_some_and(|document| document.source == artifact.path)
            || cpl
                .as_ref()
                .is_some_and(|document| document.source == artifact.path)
        {
            artifact.parser_status = Some(Status::Pass);
        }
    }
}

fn package_kind_name(kind: super::package::PackageKind) -> &'static str {
    match kind {
        super::package::PackageKind::Directory => "directory",
        super::package::PackageKind::Zip => "ZIP",
    }
}

fn profile_name(profile: ManufacturingProfile) -> &'static str {
    match profile {
        ManufacturingProfile::Generic => "generic",
        ManufacturingProfile::Jlcpcb => "JLCPCB",
    }
}

fn release_kind_name(kind: ReleaseKind) -> &'static str {
    match kind {
        ReleaseKind::Fabrication => "fabrication",
        ReleaseKind::Assembly => "assembly",
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Pass => "passed",
        Status::Warn => "passed with warnings",
        Status::Fail => "failed",
        Status::Skipped => "skipped",
    }
}
