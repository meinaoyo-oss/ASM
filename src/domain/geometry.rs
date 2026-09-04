use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::gerber::{
    GerberBounds, GerberGeometry, PcbFileRole, classify_pcb_file, parse_gerber_geometry,
    validate_excellon,
};
use super::kicad_native::{KicadDocument, KicadDocumentKind};
use super::profile::ManufacturingProfile;
use super::types::{CheckResult, Finding, Severity, Status};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GeometryConsistencyReview {
    pub source: String,
    pub pcb_source: Option<String>,
    pub profile: ManufacturingProfile,
    pub pcb_copper_layers: usize,
    pub gerber_copper_layers: usize,
    pub pcb_via_count: usize,
    pub drill_hole_count: usize,
    pub pcb_pad_count: usize,
    pub soldermask_file_count: usize,
    pub soldermask_flash_count: usize,
    pub outline: GeometryComparison,
    pub gerber_files: Vec<GerberGeometrySummary>,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub status: Status,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GeometryComparison {
    pub pcb_bounds: Option<GeometryBounds>,
    pub gerber_bounds: Option<GeometryBounds>,
    pub tolerance_mm: f64,
    pub compared: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GeometryBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberGeometrySummary {
    pub path: String,
    pub role: PcbFileRole,
    pub unit: String,
    pub segment_count: usize,
    pub arc_count: usize,
    pub region_count: usize,
    pub flash_count: usize,
    pub aperture_count: usize,
    pub aperture_macro_count: usize,
    pub polarity: String,
    pub polarity_event_count: usize,
    pub step_repeat_instances: u64,
    pub bounds: Option<GeometryBounds>,
}

pub fn review_release_geometry(
    pcb: Option<&KicadDocument>,
    files: &[(String, Vec<u8>)],
    profile: ManufacturingProfile,
    source: impl Into<String>,
) -> GeometryConsistencyReview {
    let source = source.into();
    let mut checks = Vec::new();
    let mut gerber_files = Vec::new();
    let mut outline_geometry = None;
    let mut gerber_copper_layers = 0;
    let mut soldermask_flash_count = 0;
    let mut soldermask_file_count = 0;
    let mut drill_hole_count = 0;
    let mut geometry_errors = Vec::new();
    for (path, bytes) in files {
        match classify_pcb_file(path) {
            PcbFileRole::Drill => {
                let drill = validate_excellon(bytes, path.clone());
                drill_hole_count += drill.holes.len();
                checks.push(drill.check);
            }
            role if is_geometry_role(&role) => match parse_gerber_geometry(bytes, path.clone()) {
                Ok(geometry) => {
                    if role == PcbFileRole::BoardOutline {
                        outline_geometry = merge_geometry(outline_geometry, &geometry);
                    }
                    if role.is_copper_role() {
                        gerber_copper_layers += 1;
                    }
                    if matches!(
                        role,
                        PcbFileRole::GerberSolderMaskTop | PcbFileRole::GerberSolderMaskBottom
                    ) {
                        soldermask_file_count += 1;
                        soldermask_flash_count += geometry.flashes.len();
                    }
                    geometry_errors.extend(geometry.check.findings.clone());
                    gerber_files.push(GerberGeometrySummary {
                        path: path.clone(),
                        role,
                        unit: geometry.unit,
                        segment_count: geometry.segments.len(),
                        arc_count: geometry.arcs.len(),
                        region_count: geometry.regions.len(),
                        flash_count: geometry.flashes.len(),
                        aperture_count: geometry.apertures.len(),
                        aperture_macro_count: geometry.aperture_macro_count,
                        polarity: geometry.polarity,
                        polarity_event_count: geometry.polarity_events.len(),
                        step_repeat_instances: geometry.step_repeat_instances,
                        bounds: geometry.bounds.map(GeometryBounds::from),
                    });
                }
                Err(error) => geometry_errors.push(
                    Finding::new(
                        "GEOMETRY_GERBER_PARSE_FAILED",
                        Severity::Error,
                        error.to_string(),
                    )
                    .at_path(path.clone()),
                ),
            },
            _ => {}
        }
    }
    if !geometry_errors.is_empty() {
        let mut check = CheckResult::new("geometry_parsing", "parsed Gerber/Excellon geometry");
        for finding in geometry_errors {
            check.add(finding);
        }
        checks.push(check);
    }

    let pcb_copper_layers = pcb
        .filter(|document| document.kind == KicadDocumentKind::Pcb)
        .map(|document| {
            document
                .layers
                .iter()
                .filter(|layer| is_copper_layer(layer))
                .count()
        })
        .unwrap_or(0);
    let pcb_via_count = pcb.map(|document| document.vias.len()).unwrap_or(0);
    let pcb_pad_count = pcb
        .map(|document| {
            document
                .components
                .iter()
                .map(|component| component.pads.len())
                .sum()
        })
        .unwrap_or(0);
    checks.push(check_layer_consistency(
        pcb_copper_layers,
        gerber_copper_layers,
    ));
    checks.push(check_drill_consistency(pcb_via_count, drill_hole_count));
    checks.push(check_pad_evidence(
        pcb_pad_count,
        soldermask_file_count,
        soldermask_flash_count,
    ));

    let pcb_bounds = pcb.and_then(outline_bounds).map(GeometryBounds::from);
    let gerber_bounds = outline_geometry
        .as_ref()
        .and_then(|geometry| geometry.bounds.clone())
        .map(GeometryBounds::from);
    let tolerance_mm = if profile == ManufacturingProfile::Jlcpcb {
        0.10
    } else {
        0.15
    };
    let mut outline_check =
        CheckResult::new("geometry_outline", "compared PCB and Gerber outline bounds");
    if pcb_bounds.is_none() || gerber_bounds.is_none() {
        outline_check.status = Status::Skipped;
        outline_check.summary =
            "outline bounds comparison skipped because one side has no geometry".to_owned();
        outline_check.add(Finding::new(
            "GEOMETRY_OUTLINE_COMPARISON_SKIPPED",
            Severity::Warning,
            "PCB or Gerber outline has no measurable bounds",
        ));
    } else if let (Some(pcb_bounds), Some(gerber_bounds)) = (&pcb_bounds, &gerber_bounds) {
        let max_delta = [
            (pcb_bounds.min_x - gerber_bounds.min_x).abs(),
            (pcb_bounds.max_x - gerber_bounds.max_x).abs(),
            (pcb_bounds.min_y - gerber_bounds.min_y).abs(),
            (pcb_bounds.max_y - gerber_bounds.max_y).abs(),
        ]
        .into_iter()
        .fold(0.0, f64::max);
        if max_delta > tolerance_mm {
            outline_check.add(
                Finding::new(
                    "GEOMETRY_OUTLINE_DRIFT",
                    Severity::Error,
                    "Gerber outline bounds differ from KiCad Edge.Cuts beyond the selected tolerance",
                )
                .with_detail("max_delta_mm", max_delta)
                .with_detail("tolerance_mm", tolerance_mm),
            );
        }
    }
    checks.push(outline_check);
    let status = checks
        .iter()
        .fold(Status::Pass, |status, check| status.combine(check.status));
    let findings = checks
        .iter()
        .flat_map(|check| check.findings.iter().cloned())
        .collect();
    GeometryConsistencyReview {
        source,
        pcb_source: pcb.map(|document| document.source.clone()),
        profile,
        pcb_copper_layers,
        gerber_copper_layers,
        pcb_via_count,
        drill_hole_count,
        pcb_pad_count,
        soldermask_file_count,
        soldermask_flash_count,
        outline: GeometryComparison {
            pcb_bounds,
            gerber_bounds,
            tolerance_mm,
            compared: outline_geometry.is_some(),
        },
        gerber_files,
        checks,
        findings,
        status,
    }
}

fn check_layer_consistency(pcb: usize, gerber: usize) -> CheckResult {
    let mut check = CheckResult::new(
        "geometry_layers",
        "compared PCB and Gerber copper layer counts",
    );
    if pcb == 0 || gerber == 0 {
        check.status = Status::Skipped;
        check.add(Finding::new(
            "GEOMETRY_LAYER_COMPARISON_SKIPPED",
            Severity::Warning,
            "copper layer comparison needs both PCB and Gerber layer evidence",
        ));
    } else if pcb != gerber {
        check.add(
            Finding::new(
                "GEOMETRY_COPPER_LAYER_DRIFT",
                Severity::Error,
                "PCB copper layer count differs from Gerber copper file count",
            )
            .with_detail("pcb_layers", pcb)
            .with_detail("gerber_layers", gerber),
        );
    }
    check
}

fn check_drill_consistency(pcb_vias: usize, drill_holes: usize) -> CheckResult {
    let mut check = CheckResult::new(
        "geometry_drills",
        "compared PCB vias and Excellon hole evidence",
    );
    if drill_holes == 0 {
        check.status = Status::Skipped;
        check.add(Finding::new(
            "GEOMETRY_DRILL_COMPARISON_SKIPPED",
            Severity::Warning,
            "Excellon contains no parseable hole coordinates",
        ));
    } else if pcb_vias != drill_holes {
        check.add(
            Finding::new(
                "GEOMETRY_DRILL_COUNT_DIFFERENCE",
                Severity::Warning,
                "PCB via count differs from Excellon hole coordinate count; plated pads may explain part of the difference",
            )
            .with_detail("pcb_vias", pcb_vias)
            .with_detail("drill_holes", drill_holes),
        );
    }
    check
}

fn check_pad_evidence(
    pads: usize,
    soldermask_files: usize,
    soldermask_flashes: usize,
) -> CheckResult {
    let mut check = CheckResult::new(
        "geometry_pads",
        "reviewed pad and soldermask geometry evidence",
    );
    if pads > 0 && soldermask_files == 0 {
        check.add(Finding::new(
            "GEOMETRY_SOLDERMASK_FILE_MISSING",
            Severity::Warning,
            "PCB has pads but no top/bottom soldermask Gerber file was supplied",
        ));
    } else if pads > 0 && soldermask_flashes == 0 {
        check.add(Finding::new(
            "GEOMETRY_SOLDERMASK_NO_FLASH_EVIDENCE",
            Severity::Warning,
            "soldermask Gerber files exist but contain no parseable flash evidence",
        ));
    } else if pads > 0 && soldermask_flashes < pads {
        check.add(
            Finding::new(
                "GEOMETRY_SOLDERMASK_FLASH_COUNT_LOW",
                Severity::Warning,
                "soldermask flash count is lower than PCB pad count; verify apertures and thermal/oval pads",
            )
            .with_detail("pcb_pads", pads)
            .with_detail("soldermask_flashes", soldermask_flashes),
        );
    }
    check
}

fn is_geometry_role(role: &PcbFileRole) -> bool {
    matches!(
        role,
        PcbFileRole::BoardOutline
            | PcbFileRole::GerberCopperTop
            | PcbFileRole::GerberCopperBottom
            | PcbFileRole::GerberCopperInner
            | PcbFileRole::GerberSolderMaskTop
            | PcbFileRole::GerberSolderMaskBottom
            | PcbFileRole::GerberSilkscreenTop
            | PcbFileRole::GerberSilkscreenBottom
    )
}

trait CopperRole {
    fn is_copper_role(&self) -> bool;
}

impl CopperRole for PcbFileRole {
    fn is_copper_role(&self) -> bool {
        matches!(
            self,
            Self::GerberCopperTop | Self::GerberCopperBottom | Self::GerberCopperInner
        )
    }
}

fn is_copper_layer(layer: &str) -> bool {
    let layer = layer.to_ascii_lowercase();
    layer == "f.cu" || layer == "b.cu" || (layer.starts_with("in") && layer.ends_with(".cu"))
}

fn outline_bounds(document: &KicadDocument) -> Option<GerberBounds> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for segment in &document.board_outline {
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
    bounds.map(|(min_x, max_x, min_y, max_y)| GerberBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn merge_geometry(
    current: Option<GerberGeometry>,
    next: &GerberGeometry,
) -> Option<GerberGeometry> {
    let Some(mut current) = current else {
        return Some(next.clone());
    };
    current.segments.extend(next.segments.clone());
    current.arcs.extend(next.arcs.clone());
    current.regions.extend(next.regions.clone());
    current.flashes.extend(next.flashes.clone());
    current.apertures.extend(next.apertures.clone());
    current.polarity = next.polarity.clone();
    current.polarity_events.extend(next.polarity_events.clone());
    current.aperture_macro_count += next.aperture_macro_count;
    current.step_repeat_instances = current
        .step_repeat_instances
        .max(next.step_repeat_instances);
    current.bounds = gerber_geometry_bounds(&current.segments, &current.flashes);
    Some(current)
}

fn gerber_geometry_bounds(
    segments: &[super::gerber::GerberSegment],
    flashes: &[super::gerber::GerberPoint],
) -> Option<GerberBounds> {
    let mut points = segments
        .iter()
        .flat_map(|segment| [&segment.start, &segment.end])
        .chain(flashes.iter());
    let first = points.next()?;
    let mut bounds = GerberBounds {
        min_x: first.x,
        max_x: first.x,
        min_y: first.y,
        max_y: first.y,
    };
    for point in points {
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    Some(bounds)
}

impl From<GerberBounds> for GeometryBounds {
    fn from(value: GerberBounds) -> Self {
        Self {
            min_x: value.min_x,
            max_x: value.max_x,
            min_y: value.min_y,
            max_y: value.max_y,
        }
    }
}
