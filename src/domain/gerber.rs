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
    KicadSchematic,
    KicadPcb,
    SpiceNetlist,
    Requirements,
    TraceLinks,
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
            Self::KicadSchematic => "kicad_schematic",
            Self::KicadPcb => "kicad_pcb",
            Self::SpiceNetlist => "spice_netlist",
            Self::Requirements => "requirements",
            Self::TraceLinks => "trace_links",
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
    pub holes: Vec<GerberPoint>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberSegment {
    pub start: GerberPoint,
    pub end: GerberPoint,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberArc {
    pub start: GerberPoint,
    pub end: GerberPoint,
    pub offset: GerberPoint,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberRegion {
    pub segments: Vec<GerberSegment>,
    pub closed: bool,
    pub bounds: Option<GerberBounds>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberAperture {
    pub code: i32,
    pub shape: String,
    pub x_mm: Option<f64>,
    pub y_mm: Option<f64>,
    pub hole_mm: Option<f64>,
    pub vertices: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct GerberGeometry {
    pub source: String,
    pub unit: String,
    pub segments: Vec<GerberSegment>,
    pub arcs: Vec<GerberArc>,
    pub regions: Vec<GerberRegion>,
    pub flashes: Vec<GerberPoint>,
    pub apertures: Vec<GerberAperture>,
    pub step_repeat_instances: u64,
    pub bounds: Option<GerberBounds>,
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
    if name.ends_with(".kicad_sch") {
        return PcbFileRole::KicadSchematic;
    }
    if name.ends_with(".kicad_pcb") {
        return PcbFileRole::KicadPcb;
    }
    if name.ends_with(".cir") || name.ends_with(".sp") || name.ends_with(".spice") {
        return PcbFileRole::SpiceNetlist;
    }
    if name.contains("trace") && (name.ends_with(".csv") || name.ends_with(".json")) {
        return PcbFileRole::TraceLinks;
    }
    if (name.contains("requirement") || name.contains("req"))
        && (name.ends_with(".csv") || name.ends_with(".json") || name.ends_with(".md"))
    {
        return PcbFileRole::Requirements;
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
    let holes = text
        .lines()
        .filter_map(parse_excellon_point)
        .collect::<Vec<_>>();
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
        holes,
        check,
    }
}

pub fn parse_gerber_geometry(
    bytes: &[u8],
    source: impl Into<String>,
) -> Result<GerberGeometry, super::types::DomainError> {
    use gerber_parser::gerber_types::{
        Command, DCode, ExtendedCode, FunctionCode, GCode, Operation, StepAndRepeat, Unit,
    };

    let source = source.into();
    let document =
        gerber_parser::parse(BufReader::new(Cursor::new(bytes))).map_err(|(_, error)| {
            super::types::DomainError::InvalidInput(format!("Gerber parser failed: {error}"))
        })?;
    let (unit_name, unit_scale) = match document.units.unwrap_or(Unit::Millimeters) {
        Unit::Inches => ("inches", 25.4),
        Unit::Millimeters => ("millimeters", 1.0),
    };
    let mut current = None;
    let mut segments = Vec::new();
    let mut arcs = Vec::new();
    let mut regions = Vec::new();
    let mut region_open = false;
    let mut region_segments = Vec::new();
    let mut flashes = Vec::new();
    let mut step_repeat_instances = 1_u64;
    for command in document.commands() {
        match command {
            Command::FunctionCode(FunctionCode::GCode(GCode::RegionMode(enabled))) => {
                if *enabled {
                    region_open = true;
                    region_segments.clear();
                } else if region_open {
                    let region_segments_finished = std::mem::take(&mut region_segments);
                    let closed = is_closed_segment_chain(&region_segments_finished);
                    regions.push(GerberRegion {
                        bounds: gerber_geometry_bounds(&region_segments_finished, &[]),
                        segments: region_segments_finished,
                        closed,
                    });
                    region_open = false;
                }
            }
            Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Open {
                repeat_x,
                repeat_y,
                ..
            })) => {
                step_repeat_instances =
                    step_repeat_instances.max(*repeat_x as u64 * *repeat_y as u64);
            }
            _ => {}
        }
        let operation = match command {
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(operation))) => operation,
            _ => continue,
        };
        let (coordinates, operation_kind, offset) = match operation {
            Operation::Interpolate(coordinates, offset) => {
                (coordinates, "segment", offset.as_ref())
            }
            Operation::Move(coordinates) => (coordinates, "move", None),
            Operation::Flash(coordinates) => (coordinates, "flash", None),
        };
        let Some(point) =
            gerber_coordinates_to_point(coordinates.as_ref(), current.as_ref(), unit_scale)
        else {
            continue;
        };
        match operation_kind {
            "segment" => {
                if let Some(start) = current.clone() {
                    let segment = GerberSegment {
                        start,
                        end: point.clone(),
                    };
                    if region_open {
                        region_segments.push(segment.clone());
                    }
                    segments.push(segment);
                }
                if let (Some(start), Some(offset)) = (
                    current.clone(),
                    coordinates_offset_to_point(offset, unit_scale),
                ) {
                    arcs.push(GerberArc {
                        start,
                        end: point.clone(),
                        offset,
                    });
                }
            }
            "flash" => flashes.push(point.clone()),
            _ => {}
        }
        current = Some(point);
    }
    if region_open {
        let region_segments_finished = std::mem::take(&mut region_segments);
        regions.push(GerberRegion {
            bounds: gerber_geometry_bounds(&region_segments_finished, &[]),
            closed: false,
            segments: region_segments_finished,
        });
    }
    let mut check = CheckResult::new("gerber_geometry", "extracted Gerber coordinate geometry");
    for error in document.errors() {
        check.add(
            Finding::new(
                "GERBER_GEOMETRY_PARSE_ERROR",
                Severity::Error,
                format!("Gerber geometry contains parser error: {error}"),
            )
            .at_path(source.clone()),
        );
    }
    if segments.is_empty() && flashes.is_empty() {
        check.add(
            Finding::new(
                "GERBER_NO_GEOMETRY",
                Severity::Warning,
                "Gerber file contains no coordinate segments or flashes",
            )
            .at_path(source.clone()),
        );
    }
    for region in &regions {
        if !region.closed {
            check.add(
                Finding::new(
                    "GERBER_REGION_OPEN",
                    Severity::Error,
                    "Gerber region contour is not closed",
                )
                .at_path(source.clone()),
            );
        }
    }
    let mut apertures = document
        .apertures
        .iter()
        .map(|(code, aperture)| aperture_to_model(*code, aperture, unit_scale))
        .collect::<Vec<_>>();
    apertures.sort_by_key(|aperture| aperture.code);
    if apertures.is_empty() && !flashes.is_empty() {
        check.add(
            Finding::new(
                "GERBER_APERTURES_MISSING",
                Severity::Error,
                "Gerber has flashes but no parsed aperture definitions",
            )
            .at_path(source.clone()),
        );
    }
    let bounds = gerber_geometry_bounds(&segments, &flashes);
    Ok(GerberGeometry {
        source,
        unit: unit_name.to_owned(),
        segments,
        arcs,
        regions,
        flashes,
        apertures,
        step_repeat_instances,
        bounds,
        check,
    })
}

fn coordinates_offset_to_point(
    offset: Option<&gerber_parser::gerber_types::CoordinateOffset>,
    unit_scale: f64,
) -> Option<GerberPoint> {
    let offset = offset?;
    Some(GerberPoint {
        x: offset.x.map(f64::from).unwrap_or_default() * unit_scale,
        y: offset.y.map(f64::from).unwrap_or_default() * unit_scale,
    })
}

fn is_closed_segment_chain(segments: &[GerberSegment]) -> bool {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return false;
    };
    points_equal(&first.start, &last.end)
}

fn points_equal(left: &GerberPoint, right: &GerberPoint) -> bool {
    (left.x - right.x).abs() <= 1e-6 && (left.y - right.y).abs() <= 1e-6
}

fn aperture_to_model(
    code: i32,
    aperture: &gerber_parser::gerber_types::Aperture,
    unit_scale: f64,
) -> GerberAperture {
    use gerber_parser::gerber_types::Aperture;
    match aperture {
        Aperture::Circle(circle) => GerberAperture {
            code,
            shape: "circle".to_owned(),
            x_mm: Some(circle.diameter * unit_scale),
            y_mm: None,
            hole_mm: circle.hole_diameter.map(|value| value * unit_scale),
            vertices: None,
        },
        Aperture::Rectangle(rectangle) => GerberAperture {
            code,
            shape: "rectangle".to_owned(),
            x_mm: Some(rectangle.x * unit_scale),
            y_mm: Some(rectangle.y * unit_scale),
            hole_mm: rectangle.hole_diameter.map(|value| value * unit_scale),
            vertices: None,
        },
        Aperture::Obround(rectangle) => GerberAperture {
            code,
            shape: "obround".to_owned(),
            x_mm: Some(rectangle.x * unit_scale),
            y_mm: Some(rectangle.y * unit_scale),
            hole_mm: rectangle.hole_diameter.map(|value| value * unit_scale),
            vertices: None,
        },
        Aperture::Polygon(polygon) => GerberAperture {
            code,
            shape: "polygon".to_owned(),
            x_mm: Some(polygon.diameter * unit_scale),
            y_mm: None,
            hole_mm: polygon.hole_diameter.map(|value| value * unit_scale),
            vertices: Some(polygon.vertices),
        },
        Aperture::Macro(name, _) => GerberAperture {
            code,
            shape: format!("macro:{name}"),
            x_mm: None,
            y_mm: None,
            hole_mm: None,
            vertices: None,
        },
    }
}

fn gerber_coordinates_to_point(
    coordinates: Option<&gerber_parser::gerber_types::Coordinates>,
    current: Option<&GerberPoint>,
    unit_scale: f64,
) -> Option<GerberPoint> {
    let coordinates = coordinates?;
    let x = coordinates
        .x
        .map(f64::from)
        .or_else(|| current.map(|point| point.x / unit_scale))?
        * unit_scale;
    let y = coordinates
        .y
        .map(f64::from)
        .or_else(|| current.map(|point| point.y / unit_scale))?
        * unit_scale;
    Some(GerberPoint { x, y })
}

fn gerber_geometry_bounds(
    segments: &[GerberSegment],
    flashes: &[GerberPoint],
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

fn parse_excellon_point(line: &str) -> Option<GerberPoint> {
    let line = line.trim().to_ascii_uppercase();
    if !line.contains('X') || !line.contains('Y') {
        return None;
    }
    let x_start = line.find('X')? + 1;
    let y_start = line.find('Y')? + 1;
    let x_end = line[x_start..]
        .find(['Y', 'I', 'J'])
        .map(|offset| x_start + offset)
        .unwrap_or(y_start - 1);
    let y_end = line[y_start..]
        .find(['X', 'I', 'J'])
        .map(|offset| y_start + offset)
        .unwrap_or(line.len());
    let x = line[x_start..x_end].parse::<f64>().ok()?;
    let y = line[y_start..y_end].parse::<f64>().ok()?;
    Some(GerberPoint { x, y })
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
