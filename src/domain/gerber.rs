use std::io::{BufReader, Cursor};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::profile::ManufacturingProfile;
use super::types::{CheckResult, Finding, Severity, Status};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcbFileRole {
    GerberCopperTop,
    GerberCopperBottom,
    GerberCopperInner,
    GerberSolderMaskTop,
    GerberSolderMaskBottom,
    GerberSilkscreenTop,
    GerberSilkscreenBottom,
    BoardOutline,
    Drill,
    Bom,
    Cpl,
    Ipc2581,
    Unknown,
}

impl PcbFileRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GerberCopperTop => "gerber_copper_top",
            Self::GerberCopperBottom => "gerber_copper_bottom",
            Self::GerberCopperInner => "gerber_copper_inner",
            Self::GerberSolderMaskTop => "gerber_solder_mask_top",
            Self::GerberSolderMaskBottom => "gerber_solder_mask_bottom",
            Self::GerberSilkscreenTop => "gerber_silkscreen_top",
            Self::GerberSilkscreenBottom => "gerber_silkscreen_bottom",
            Self::BoardOutline => "board_outline",
            Self::Drill => "drill",
            Self::Bom => "bom",
            Self::Cpl => "cpl",
            Self::Ipc2581 => "ipc2581",
            Self::Unknown => "unknown",
        }
    }

    fn is_gerber(&self) -> bool {
        matches!(
            self,
            Self::GerberCopperTop
                | Self::GerberCopperBottom
                | Self::GerberCopperInner
                | Self::GerberSolderMaskTop
                | Self::GerberSolderMaskBottom
                | Self::GerberSilkscreenTop
                | Self::GerberSilkscreenBottom
                | Self::BoardOutline
        )
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberFileResult {
    pub path: String,
    pub role: PcbFileRole,
    pub status: Status,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberValidation {
    pub files: Vec<GerberFileResult>,
    pub copper_layer_count: u8,
    pub has_board_outline: bool,
    pub has_drill: bool,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ExcellonValidation {
    pub source: String,
    pub tool_count: usize,
    pub coordinate_count: usize,
    pub check: CheckResult,
}

/// Identifies conventional release files by name and extension.  It is a
/// deterministic heuristic, not an assertion about the internal data.
pub fn classify_pcb_file(path: &str) -> PcbFileRole {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if name.ends_with(".ipc") || name.ends_with(".ipc2581") || name.ends_with(".ipc-2581") {
        return PcbFileRole::Ipc2581;
    }
    if name.ends_with(".drl") || name.ends_with(".xln") || name.ends_with(".exc") {
        return PcbFileRole::Drill;
    }
    if name.ends_with(".gko")
        || name.ends_with(".gm1")
        || name.ends_with(".gml")
        || name.contains("edge.cuts")
        || name.contains("outline")
    {
        return PcbFileRole::BoardOutline;
    }
    if name.ends_with(".gtl") || name.contains("f.cu") || name.contains("top_copper") {
        return PcbFileRole::GerberCopperTop;
    }
    if name.ends_with(".gbl") || name.contains("b.cu") || name.contains("bottom_copper") {
        return PcbFileRole::GerberCopperBottom;
    }
    if is_inner_copper_name(name) {
        return PcbFileRole::GerberCopperInner;
    }
    if name.ends_with(".gts") || name.contains("f.mask") || name.contains("top_solder") {
        return PcbFileRole::GerberSolderMaskTop;
    }
    if name.ends_with(".gbs") || name.contains("b.mask") || name.contains("bottom_solder") {
        return PcbFileRole::GerberSolderMaskBottom;
    }
    if name.ends_with(".gto") || name.contains("f.silk") || name.contains("top_silkscreen") {
        return PcbFileRole::GerberSilkscreenTop;
    }
    if name.ends_with(".gbo") || name.contains("b.silk") || name.contains("bottom_silkscreen") {
        return PcbFileRole::GerberSilkscreenBottom;
    }
    if name.ends_with(".csv") || name.ends_with(".tsv") {
        if name.contains("cpl")
            || name.contains("pnp")
            || name.contains("pick")
            || name.contains("position")
        {
            return PcbFileRole::Cpl;
        }
        if name.contains("bom") || name.contains("component") {
            return PcbFileRole::Bom;
        }
    }
    PcbFileRole::Unknown
}

pub fn validate_gerber_set(
    files: &[(String, Vec<u8>)],
    profile: ManufacturingProfile,
    expected_copper_layers: Option<u8>,
) -> GerberValidation {
    let rules = profile.rules();
    let mut check = CheckResult::new("gerber_set", "validated Gerber layer set and basic syntax");
    let mut results = Vec::new();
    let mut top_copper_count = 0_u8;
    let mut bottom_copper_count = 0_u8;
    let mut inner_copper_count = 0_u8;
    let mut has_outline = false;
    let mut has_drill = false;

    for (path, bytes) in files {
        let role = classify_pcb_file(path);
        if role == PcbFileRole::Drill {
            has_drill = true;
            let drill = validate_excellon(bytes, path);
            check.findings.extend(drill.check.findings);
            check.status = check.status.combine(drill.check.status);
            results.push(GerberFileResult {
                path: path.clone(),
                role,
                status: drill.check.status,
            });
            continue;
        }
        if !role.is_gerber() {
            continue;
        }
        if role == PcbFileRole::BoardOutline {
            has_outline = true;
        }
        match role {
            PcbFileRole::GerberCopperTop => top_copper_count = top_copper_count.saturating_add(1),
            PcbFileRole::GerberCopperBottom => {
                bottom_copper_count = bottom_copper_count.saturating_add(1)
            }
            PcbFileRole::GerberCopperInner => {
                inner_copper_count = inner_copper_count.saturating_add(1)
            }
            _ => {}
        }
        let file_check = validate_gerber_file(bytes, path);
        check.findings.extend(file_check.findings);
        check.status = check.status.combine(file_check.status);
        results.push(GerberFileResult {
            path: path.clone(),
            role,
            status: file_check.status,
        });
    }

    if top_copper_count > 1 {
        check.add(Finding::new(
            "GERBER_DUPLICATE_TOP_COPPER",
            Severity::Error,
            "Gerber release contains more than one recognizable top-copper file",
        ));
    }
    if bottom_copper_count > 1 {
        check.add(Finding::new(
            "GERBER_DUPLICATE_BOTTOM_COPPER",
            Severity::Error,
            "Gerber release contains more than one recognizable bottom-copper file",
        ));
    }
    let copper_layer_count = u8::from(top_copper_count > 0)
        .saturating_add(u8::from(bottom_copper_count > 0))
        .saturating_add(inner_copper_count);
    let required_copper_layers = match expected_copper_layers {
        Some(0) => {
            check.add(Finding::new(
                "GERBER_INVALID_EXPECTED_LAYER_COUNT",
                Severity::Error,
                "expected_copper_layers must be greater than zero",
            ));
            rules.minimum_copper_layers
        }
        Some(value) => value,
        None => rules.minimum_copper_layers,
    };
    if copper_layer_count < required_copper_layers {
        check.add(Finding::new(
            "GERBER_INSUFFICIENT_COPPER_LAYERS",
            Severity::Error,
            format!("found {copper_layer_count} copper layers; expected at least {required_copper_layers}"),
        ));
    }
    if rules.require_board_outline && !has_outline {
        check.add(Finding::new(
            "GERBER_MISSING_BOARD_OUTLINE",
            Severity::Error,
            "Gerber release has no recognizable board-outline file",
        ));
    }
    if rules.require_drill && !has_drill {
        check.add(Finding::new(
            "GERBER_MISSING_DRILL",
            Severity::Error,
            "Gerber release has no recognizable Excellon drill file",
        ));
    }
    results.sort_by(|left, right| left.path.cmp(&right.path));
    GerberValidation {
        files: results,
        copper_layer_count,
        has_board_outline: has_outline,
        has_drill,
        check,
    }
}

pub fn validate_excellon(bytes: &[u8], source: impl Into<String>) -> ExcellonValidation {
    let source = source.into();
    let text = String::from_utf8_lossy(bytes).to_ascii_uppercase();
    let mut check = CheckResult::new("excellon", "validated basic Excellon drill syntax");
    if text.trim().is_empty() {
        check.add(
            Finding::new("EXCELLON_EMPTY", Severity::Error, "drill file is empty")
                .at_path(source.clone()),
        );
    }
    if !text.contains("M48") {
        check.add(
            Finding::new(
                "EXCELLON_MISSING_HEADER",
                Severity::Error,
                "drill file has no M48 header",
            )
            .at_path(source.clone()),
        );
    }
    if !text.contains("M30") {
        check.add(
            Finding::new(
                "EXCELLON_MISSING_TERMINATOR",
                Severity::Error,
                "drill file has no M30 terminator",
            )
            .at_path(source.clone()),
        );
    }
    let tool_count = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('T') && trimmed.contains('C')
        })
        .count();
    let coordinate_count = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('X') || (trimmed.starts_with('Y') && trimmed.contains('X'))
        })
        .count();
    if tool_count == 0 {
        check.add(
            Finding::new(
                "EXCELLON_NO_TOOLS",
                Severity::Error,
                "drill file defines no tools",
            )
            .at_path(source.clone()),
        );
    }
    if coordinate_count == 0 {
        check.add(
            Finding::new(
                "EXCELLON_NO_COORDINATES",
                Severity::Warning,
                "drill file contains no recognizable coordinates",
            )
            .at_path(source.clone()),
        );
    }
    ExcellonValidation {
        source,
        tool_count,
        coordinate_count,
        check,
    }
}

fn validate_gerber_file(bytes: &[u8], source: &str) -> CheckResult {
    let text = String::from_utf8_lossy(bytes);
    let mut check = CheckResult::new("gerber_file", "validated basic Gerber syntax");
    if text.trim().is_empty() {
        check.add(
            Finding::new("GERBER_EMPTY", Severity::Error, "Gerber file is empty").at_path(source),
        );
        return check;
    }
    // gerber_parser parses RS-274X and records content-level failures on the
    // document, while fatal reader failures are returned as Err.
    match gerber_parser::parse(BufReader::new(Cursor::new(bytes))) {
        Ok(document) => {
            for error in document.errors() {
                check.add(
                    Finding::new(
                        "GERBER_PARSE_FAILED",
                        Severity::Error,
                        format!("Gerber parser rejected content: {error}"),
                    )
                    .at_path(source),
                );
            }
        }
        Err((_partial_document, error)) => check.add(
            Finding::new(
                "GERBER_PARSE_FAILED",
                Severity::Error,
                format!("Gerber parser failed: {error}"),
            )
            .at_path(source),
        ),
    }
    let parameter_delimiters = text.bytes().filter(|byte| *byte == b'%').count();
    if parameter_delimiters % 2 != 0 {
        check.add(
            Finding::new(
                "GERBER_UNBALANCED_PARAMETER",
                Severity::Error,
                "Gerber parameter blocks have an unmatched percent delimiter",
            )
            .at_path(source),
        );
    }
    if !text.contains("M02*") {
        check.add(
            Finding::new(
                "GERBER_MISSING_TERMINATOR",
                Severity::Warning,
                "Gerber file has no M02* terminator",
            )
            .at_path(source),
        );
    }
    if !text.contains("%ADD") {
        check.add(
            Finding::new(
                "GERBER_NO_APERTURE",
                Severity::Warning,
                "Gerber file has no aperture definition",
            )
            .at_path(source),
        );
    }
    check
}

fn is_inner_copper_name(name: &str) -> bool {
    name.ends_with(".g1")
        || name.ends_with(".g2")
        || name.ends_with(".g3")
        || name.ends_with(".g4")
        || name.ends_with(".g5")
        || name.ends_with(".g6")
        || name.ends_with(".g7")
        || name.ends_with(".g8")
        || name.contains("in1.cu")
        || name.contains("in2.cu")
        || name.contains("inner")
}
