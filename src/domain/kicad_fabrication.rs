use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::kicad_native::{KicadDocumentKind, KicadPoint, KicadProjectSnapshot};
use super::profile::ManufacturingProfile;
use super::types::{CheckResult, Finding, Severity, Status};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct PcbDfmDfaDftReview {
    pub source: String,
    pub profile: ManufacturingProfile,
    pub thresholds: DfmThresholds,
    pub metrics: BTreeMap<String, usize>,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub status: Status,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct DfmThresholds {
    pub min_trace_width_mm: f64,
    pub min_edge_clearance_mm: f64,
    pub min_drill_mm: f64,
    pub min_copper_layers: u8,
}

impl DfmThresholds {
    pub fn for_profile(profile: ManufacturingProfile) -> Self {
        match profile {
            ManufacturingProfile::Generic => Self {
                min_trace_width_mm: 0.15,
                min_edge_clearance_mm: 0.20,
                min_drill_mm: 0.20,
                min_copper_layers: 2,
            },
            ManufacturingProfile::Jlcpcb => Self {
                min_trace_width_mm: 0.10,
                min_edge_clearance_mm: 0.20,
                min_drill_mm: 0.20,
                min_copper_layers: 2,
            },
        }
    }
}

pub fn review_pcb_dfm_dfa_dft(
    project: &KicadProjectSnapshot,
    profile: ManufacturingProfile,
) -> PcbDfmDfaDftReview {
    let thresholds = DfmThresholds::for_profile(profile);
    let mut checks = Vec::new();
    let mut metrics = BTreeMap::new();
    let Some(board) = project
        .documents
        .iter()
        .find(|document| document.kind == KicadDocumentKind::Pcb)
    else {
        let mut check = CheckResult::skipped(
            "pcb_dfm",
            "no PCB document was available for DFM/DFA/DFT review",
        );
        check.add(Finding::new(
            "PCB_DFM_NO_BOARD",
            Severity::Warning,
            "DFM/DFA/DFT review needs a KiCad PCB document",
        ));
        checks.push(check);
        return finalize(project.source.clone(), profile, thresholds, metrics, checks);
    };

    let copper_layers = board
        .layers
        .iter()
        .filter(|layer| is_copper_layer(layer))
        .count();
    metrics.insert("copper_layers".to_owned(), copper_layers);
    metrics.insert("outline_segments".to_owned(), board.board_outline.len());
    metrics.insert("track_segments".to_owned(), board.tracks.len());
    metrics.insert("vias".to_owned(), board.vias.len());
    metrics.insert(
        "pads".to_owned(),
        board
            .components
            .iter()
            .map(|component| component.pads.len())
            .sum(),
    );
    metrics.insert("footprints".to_owned(), board.components.len());

    checks.push(check_layers(
        board,
        copper_layers,
        thresholds.min_copper_layers,
    ));
    checks.push(check_outline(board));
    checks.push(check_tracks(board, thresholds.min_trace_width_mm));
    checks.push(check_vias(board, thresholds.min_drill_mm));
    checks.push(check_assembly(board, thresholds.min_edge_clearance_mm));
    checks.push(check_test_access(board));
    finalize(project.source.clone(), profile, thresholds, metrics, checks)
}

fn finalize(
    source: String,
    profile: ManufacturingProfile,
    thresholds: DfmThresholds,
    metrics: BTreeMap<String, usize>,
    checks: Vec<CheckResult>,
) -> PcbDfmDfaDftReview {
    let status = checks
        .iter()
        .fold(Status::Pass, |status, check| status.combine(check.status));
    let findings = checks
        .iter()
        .flat_map(|check| check.findings.iter().cloned())
        .collect();
    PcbDfmDfaDftReview {
        source,
        profile,
        thresholds,
        metrics,
        checks,
        findings,
        status,
    }
}

fn check_layers(
    board: &super::kicad_native::KicadDocument,
    copper_layers: usize,
    minimum: u8,
) -> CheckResult {
    let mut check = CheckResult::new("pcb_dfm_layers", "reviewed PCB copper layer set");
    if copper_layers < minimum as usize {
        check.add(Finding::new(
            "PCB_DFM_INSUFFICIENT_COPPER_LAYERS",
            Severity::Error,
            format!("PCB has {copper_layers} copper layers; at least {minimum} are required"),
        ));
    }
    if !board
        .layers
        .iter()
        .any(|layer| layer.eq_ignore_ascii_case("F.Cu"))
    {
        check.add(Finding::new(
            "PCB_DFM_MISSING_TOP_COPPER",
            Severity::Error,
            "PCB layer table has no F.Cu layer",
        ));
    }
    if !board
        .layers
        .iter()
        .any(|layer| layer.eq_ignore_ascii_case("B.Cu"))
    {
        check.add(Finding::new(
            "PCB_DFM_MISSING_BOTTOM_COPPER",
            Severity::Error,
            "PCB layer table has no B.Cu layer",
        ));
    }
    check
}

fn check_outline(board: &super::kicad_native::KicadDocument) -> CheckResult {
    let mut check = CheckResult::new("pcb_dfm_outline", "reviewed Edge.Cuts board outline");
    if board.board_outline.is_empty() {
        check.add(Finding::new(
            "PCB_DFM_MISSING_OUTLINE",
            Severity::Error,
            "PCB has no recognizable Edge.Cuts outline segments",
        ));
        return check;
    }
    let mut degree = BTreeMap::<PointKey, usize>::new();
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for segment in &board.board_outline {
        let start = PointKey::from(&segment.start);
        let end = PointKey::from(&segment.end);
        *degree.entry(start).or_default() += 1;
        *degree.entry(end).or_default() += 1;
        for point in [&segment.start, &segment.end] {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
    }
    let open_points = degree.values().filter(|count| **count != 2).count();
    if open_points > 0 {
        check.add(
            Finding::new(
                "PCB_DFM_OUTLINE_OPEN",
                Severity::Error,
                "Edge.Cuts outline is not a closed endpoint graph",
            )
            .with_detail("open_or_branch_points", open_points),
        );
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        check.add(Finding::new(
            "PCB_DFM_OUTLINE_DEGENERATE",
            Severity::Error,
            "Edge.Cuts outline has zero or invalid extent",
        ));
    }
    check
}

fn check_tracks(board: &super::kicad_native::KicadDocument, minimum: f64) -> CheckResult {
    let mut check = CheckResult::new("pcb_dfm_tracks", "reviewed copper track widths");
    let mut missing_width = 0;
    for track in &board.tracks {
        match track.width {
            Some(width) if width.is_finite() && width >= minimum => {}
            Some(width) if width.is_finite() => check.add(
                Finding::new(
                    "PCB_DFM_TRACE_TOO_NARROW",
                    Severity::Error,
                    "copper track width is below the selected precheck threshold",
                )
                .with_detail("width_mm", width)
                .with_detail("minimum_mm", minimum)
                .with_detail("layer", track.layer.clone().unwrap_or_default()),
            ),
            _ => missing_width += 1,
        }
    }
    if missing_width > 0 {
        check.add(
            Finding::new(
                "PCB_DFM_TRACE_WIDTH_UNKNOWN",
                Severity::Warning,
                "one or more copper tracks have no usable width value",
            )
            .with_detail("count", missing_width),
        );
    }
    check
}

fn check_vias(board: &super::kicad_native::KicadDocument, minimum_drill: f64) -> CheckResult {
    let mut check = CheckResult::new("pcb_dfm_vias", "reviewed via size and drill values");
    for via in &board.vias {
        match (via.size, via.drill) {
            (Some(size), Some(drill))
                if size.is_finite()
                    && drill.is_finite()
                    && size >= drill
                    && drill >= minimum_drill => {}
            (Some(size), Some(drill)) if size.is_finite() && drill.is_finite() && size < drill => {
                check.add(
                    Finding::new(
                        "PCB_DFM_VIA_DRILL_EXCEEDS_SIZE",
                        Severity::Error,
                        "via drill is larger than via diameter",
                    )
                    .with_detail("size_mm", size)
                    .with_detail("drill_mm", drill),
                )
            }
            (Some(_), Some(drill)) if drill.is_finite() => check.add(
                Finding::new(
                    "PCB_DFM_DRILL_TOO_SMALL",
                    Severity::Warning,
                    "via drill is below the selected precheck threshold",
                )
                .with_detail("drill_mm", drill)
                .with_detail("minimum_mm", minimum_drill),
            ),
            _ => check.add(Finding::new(
                "PCB_DFM_VIA_DIMENSION_UNKNOWN",
                Severity::Warning,
                "via has missing or invalid size/drill values",
            )),
        }
    }
    check
}

fn check_assembly(board: &super::kicad_native::KicadDocument, minimum_edge: f64) -> CheckResult {
    let mut check = CheckResult::new(
        "pcb_dfa_assembly",
        "reviewed footprint pads and board-edge clearance",
    );
    let Some((min_x, max_x, min_y, max_y)) = outline_bounds(board) else {
        check.add(Finding::new(
            "PCB_DFA_EDGE_CLEARANCE_SKIPPED",
            Severity::Warning,
            "board-edge clearance was skipped because Edge.Cuts bounds are unavailable",
        ));
        return check;
    };
    let mut positions = Vec::new();
    for component in &board.components {
        if component.pads.is_empty() {
            check.add(
                Finding::new(
                    "PCB_DFA_FOOTPRINT_NO_PADS",
                    Severity::Warning,
                    "footprint has no recognizable pads for assembly/test review",
                )
                .with_detail("reference", component.reference.clone()),
            );
        }
        if let (Some(x), Some(y)) = (component.x, component.y) {
            positions.push((component.reference.clone(), x, y));
        }
        for pad in &component.pads {
            let (Some(x), Some(y)) = (pad.x, pad.y) else {
                continue;
            };
            let clearance = (x - min_x).min(max_x - x).min((y - min_y).min(max_y - y));
            if clearance < minimum_edge {
                check.add(
                    Finding::new(
                        "PCB_DFA_PAD_EDGE_CLEARANCE",
                        Severity::Error,
                        "pad is closer to the board edge than the selected precheck threshold",
                    )
                    .with_detail("reference", component.reference.clone())
                    .with_detail("pad", pad.number.clone())
                    .with_detail("clearance_mm", clearance)
                    .with_detail("minimum_mm", minimum_edge),
                );
            }
        }
    }
    for (index, (left_reference, left_x, left_y)) in positions.iter().enumerate() {
        for (right_reference, right_x, right_y) in positions.iter().skip(index + 1) {
            let distance = ((*left_x - *right_x).powi(2) + (*left_y - *right_y).powi(2)).sqrt();
            if distance < 0.10 {
                check.add(
                    Finding::new(
                        "PCB_DFA_COMPONENT_OVERLAP_CANDIDATE",
                        Severity::Warning,
                        "two footprint origins are within 0.10 mm; verify package overlap",
                    )
                    .with_detail("left", left_reference.clone())
                    .with_detail("right", right_reference.clone())
                    .with_detail("distance_mm", distance),
                );
            }
        }
    }
    check
}

fn check_test_access(board: &super::kicad_native::KicadDocument) -> CheckResult {
    let mut check = CheckResult::new("pcb_dft_access", "reviewed basic test access evidence");
    let test_like = board
        .components
        .iter()
        .flat_map(|component| component.pads.iter().map(move |pad| (component, pad)))
        .filter(|(component, pad)| {
            pad.net.is_some()
                && component
                    .reference
                    .chars()
                    .next()
                    .is_some_and(|value| matches!(value, 'T' | 'J'))
        })
        .count();
    if board.components.is_empty() {
        check.status = Status::Skipped;
        return check;
    }
    if test_like == 0 {
        check.add(Finding::new(
            "PCB_DFT_NO_TEST_ACCESS_CANDIDATE",
            Severity::Info,
            "no net-connected testpoint or connector candidate was identified",
        ));
    }
    check
}

fn outline_bounds(board: &super::kicad_native::KicadDocument) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for segment in &board.board_outline {
        for point in [&segment.start, &segment.end] {
            bounds = Some(match bounds {
                Some((min_x, max_x, min_y, max_y)) => (
                    min_x.min(point.x),
                    max_x.max(point.x),
                    min_y.min(point.y),
                    max_y.max(point.y),
                ),
                None => (point.x, point.x, point.y, point.y),
            });
        }
    }
    bounds
}

fn is_copper_layer(layer: &str) -> bool {
    let layer = layer.to_ascii_lowercase();
    layer == "f.cu" || layer == "b.cu" || (layer.starts_with("in") && layer.ends_with(".cu"))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PointKey(i64, i64);

impl From<&KicadPoint> for PointKey {
    fn from(point: &KicadPoint) -> Self {
        Self(
            (point.x * 1_000_000.0).round() as i64,
            (point.y * 1_000_000.0).round() as i64,
        )
    }
}
