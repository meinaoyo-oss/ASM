use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{CheckResult, DomainError, DomainResult, Finding, Severity, Status};

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KicadDocumentKind {
    Schematic,
    Pcb,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadComponent {
    pub reference: String,
    pub value: Option<String>,
    pub footprint: Option<String>,
    pub library_id: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub layer: Option<String>,
    pub pads: Vec<KicadPad>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadNet {
    pub code: Option<i64>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadPad {
    pub number: String,
    pub net: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadTrack {
    pub start: KicadPoint,
    pub end: KicadPoint,
    pub width: Option<f64>,
    pub layer: Option<String>,
    pub net_code: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadVia {
    pub point: KicadPoint,
    pub size: Option<f64>,
    pub drill: Option<f64>,
    pub layers: Vec<String>,
    pub net_code: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadWire {
    pub start: KicadPoint,
    pub end: KicadPoint,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadLabel {
    pub name: String,
    pub point: KicadPoint,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadPin {
    pub reference: Option<String>,
    pub number: String,
    pub name: Option<String>,
    pub electrical_type: Option<String>,
    pub point: Option<KicadPoint>,
    pub coordinate_source: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadConnectivityNet {
    pub id: String,
    pub labels: Vec<String>,
    pub pins: Vec<KicadPin>,
    pub wire_point_count: usize,
    pub no_connect_count: usize,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadConnectivity {
    pub wire_count: usize,
    pub junction_count: usize,
    pub no_connect_count: usize,
    pub pin_count: usize,
    pub nets: Vec<KicadConnectivityNet>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadDocument {
    pub source: String,
    pub kind: KicadDocumentKind,
    pub version: Option<String>,
    pub components: Vec<KicadComponent>,
    pub nets: Vec<KicadNet>,
    pub labels: Vec<String>,
    pub label_points: Vec<KicadLabel>,
    pub layers: Vec<String>,
    pub wires: Vec<KicadWire>,
    pub junctions: Vec<KicadPoint>,
    pub no_connects: Vec<KicadPoint>,
    pub pins: Vec<KicadPin>,
    pub board_outline: Vec<KicadWire>,
    pub tracks: Vec<KicadTrack>,
    pub vias: Vec<KicadVia>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadProjectSnapshot {
    pub source: String,
    pub documents: Vec<KicadDocument>,
    pub component_count: usize,
    pub net_count: usize,
    pub label_count: usize,
    pub layer_count: usize,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadSignalTrace {
    pub query: String,
    pub matched_components: Vec<KicadComponent>,
    pub matched_nets: Vec<KicadNet>,
    pub matched_labels: Vec<String>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadPowerTree {
    pub power_nets: Vec<String>,
    pub ground_nets: Vec<String>,
    pub supply_components: Vec<KicadComponent>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadRevisionDiff {
    pub left_source: String,
    pub right_source: String,
    pub added_components: Vec<String>,
    pub removed_components: Vec<String>,
    pub changed_components: Vec<String>,
    pub added_nets: Vec<String>,
    pub removed_nets: Vec<String>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadConsistencyReport {
    pub schematic_source: Option<String>,
    pub pcb_source: Option<String>,
    pub missing_on_pcb: Vec<String>,
    pub missing_in_schematic: Vec<String>,
    pub pin_pad_mismatches: Vec<KicadPinPadMismatch>,
    pub footprint_drift: Vec<String>,
    pub net_drift: Vec<String>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadPinPadMismatch {
    pub reference: String,
    pub pin_number: String,
    pub schematic_net: Option<String>,
    pub pcb_net: Option<String>,
    pub kind: String,
}

pub fn parse_kicad_document(
    bytes: &[u8],
    source: impl Into<String>,
) -> DomainResult<KicadDocument> {
    let source = source.into();
    let text = std::str::from_utf8(bytes)
        .map_err(|error| DomainError::InvalidInput(format!("KiCad 文件不是 UTF-8: {error}")))?;
    let root = parse_sexpr(text)
        .map_err(|error| DomainError::InvalidInput(format!("KiCad S-expression 无效: {error}")))?;
    let head = list_head(&root).unwrap_or_default();
    let kind = match head {
        "kicad_sch" => KicadDocumentKind::Schematic,
        "kicad_pcb" => KicadDocumentKind::Pcb,
        _ => {
            return Err(DomainError::InvalidInput(format!(
                "不支持的 KiCad 根元素: {head}"
            )));
        }
    };
    let version = find_direct_value(&root, "version");
    let mut components = Vec::new();
    let mut nets = Vec::new();
    let mut labels = BTreeSet::new();
    let mut label_points = Vec::new();
    let mut layers = BTreeSet::new();
    collect_document_data(
        &root,
        kind,
        &mut components,
        &mut nets,
        &mut labels,
        &mut layers,
    );
    let (wires, junctions, no_connects, pins) = if kind == KicadDocumentKind::Schematic {
        collect_schematic_connectivity(&root, &mut labels, &mut label_points)
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };
    let (board_outline, tracks, vias) = if kind == KicadDocumentKind::Pcb {
        collect_pcb_geometry(&root)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    components.sort_by(|left, right| left.reference.cmp(&right.reference));
    nets.sort_by(|left, right| left.name.cmp(&right.name));
    let mut check = CheckResult::new(
        "kicad_native",
        format!(
            "parsed KiCad {:?}: {} components, {} nets",
            kind,
            components.len(),
            nets.len()
        ),
    );
    let mut references = BTreeMap::<String, usize>::new();
    for component in &components {
        let normalized = component.reference.trim().to_ascii_uppercase();
        if normalized.is_empty() || normalized == "?" || normalized.contains("?") {
            check.add(
                Finding::new(
                    "KICAD_UNANNOTATED_COMPONENT",
                    Severity::Error,
                    "KiCad component has no stable annotated reference",
                )
                .at_path(source.clone())
                .with_detail("reference", component.reference.clone()),
            );
        } else if let Some(previous) = references.insert(normalized, 0) {
            check.add(
                Finding::new(
                    "KICAD_DUPLICATE_REFERENCE",
                    Severity::Error,
                    "KiCad component reference occurs more than once",
                )
                .at_path(source.clone())
                .with_detail("reference", component.reference.clone())
                .with_detail("previous_index", previous),
            );
        } else {
            let index = references.len().saturating_sub(1);
            references.insert(component.reference.trim().to_ascii_uppercase(), index);
        }
        if component.value.as_deref().is_none_or(str::is_empty) {
            check.add(
                Finding::new(
                    "KICAD_MISSING_VALUE",
                    Severity::Warning,
                    "KiCad component has no value",
                )
                .at_path(source.clone())
                .with_detail("reference", component.reference.clone()),
            );
        }
        if kind == KicadDocumentKind::Pcb
            && component.footprint.as_deref().is_none_or(str::is_empty)
        {
            check.add(
                Finding::new(
                    "KICAD_MISSING_FOOTPRINT",
                    Severity::Warning,
                    "PCB footprint has no footprint identifier",
                )
                .at_path(source.clone())
                .with_detail("reference", component.reference.clone()),
            );
        }
    }
    if components.is_empty() {
        check.add(
            Finding::new(
                "KICAD_NO_COMPONENTS",
                Severity::Warning,
                "KiCad document contains no recognizable components",
            )
            .at_path(source.clone()),
        );
    }
    if kind == KicadDocumentKind::Schematic
        && nets.is_empty()
        && labels.is_empty()
        && wires.is_empty()
    {
        check.add(
            Finding::new(
                "KICAD_NO_NET_EVIDENCE",
                Severity::Warning,
                "schematic contains no explicit nets or labels for connectivity review",
            )
            .at_path(source.clone()),
        );
    }
    Ok(KicadDocument {
        source,
        kind,
        version,
        components,
        nets,
        labels: labels.into_iter().collect(),
        label_points,
        layers: layers.into_iter().collect(),
        wires,
        junctions,
        no_connects,
        pins,
        board_outline,
        tracks,
        vias,
        check,
    })
}

pub fn analyze_kicad_connectivity(document: &KicadDocument) -> KicadConnectivity {
    let mut check = CheckResult::new(
        "kicad_connectivity",
        format!(
            "analyzed {} wires and {} identifiable pins",
            document.wires.len(),
            document.pins.len()
        ),
    );
    if document.kind != KicadDocumentKind::Schematic {
        check.status = Status::Skipped;
        check.summary = "connectivity analysis is only available for KiCad schematics".to_owned();
        return KicadConnectivity {
            wire_count: 0,
            junction_count: 0,
            no_connect_count: 0,
            pin_count: 0,
            nets: Vec::new(),
            check,
        };
    }

    let mut points = BTreeSet::<PointKey>::new();
    let mut endpoint_degrees = BTreeMap::<PointKey, usize>::new();
    for wire in &document.wires {
        points.insert(PointKey::from(&wire.start));
        points.insert(PointKey::from(&wire.end));
        *endpoint_degrees
            .entry(PointKey::from(&wire.start))
            .or_default() += 1;
        *endpoint_degrees
            .entry(PointKey::from(&wire.end))
            .or_default() += 1;
    }
    for point in &document.junctions {
        points.insert(PointKey::from(point));
    }
    for point in &document.no_connects {
        points.insert(PointKey::from(point));
    }
    for label in &document.label_points {
        points.insert(PointKey::from(&label.point));
    }
    for pin in &document.pins {
        if let Some(point) = &pin.point {
            points.insert(PointKey::from(point));
        }
    }
    let points = points.into_iter().collect::<Vec<_>>();
    let mut index = BTreeMap::<PointKey, usize>::new();
    for (position, point) in points.iter().copied().enumerate() {
        index.insert(point, position);
    }
    let mut dsu = DisjointSet::new(points.len());
    for wire in &document.wires {
        let start = PointKey::from(&wire.start);
        let end = PointKey::from(&wire.end);
        if let (Some(start), Some(end)) = (index.get(&start), index.get(&end)) {
            dsu.union(*start, *end);
        }
    }
    let mut builders = BTreeMap::<usize, ConnectivityBuilder>::new();
    for (point_index, point) in points.iter().copied().enumerate() {
        let root = dsu.find(point_index);
        builders.entry(root).or_default().wire_point_count += 1;
        if endpoint_degrees.get(&point).copied() == Some(1) {
            builders.entry(root).or_default().dangling_endpoint_count += 1;
        }
        if document
            .junctions
            .iter()
            .any(|junction| PointKey::from(junction) == point)
        {
            builders.entry(root).or_default().junction_count += 1;
        }
        if document
            .no_connects
            .iter()
            .any(|no_connect| PointKey::from(no_connect) == point)
        {
            builders.entry(root).or_default().no_connect_count += 1;
        }
    }
    for label in &document.label_points {
        if let Some(point_index) = index.get(&PointKey::from(&label.point)) {
            builders
                .entry(dsu.find(*point_index))
                .or_default()
                .labels
                .insert(label.name.clone());
        }
    }
    for pin in &document.pins {
        if let Some(point) = &pin.point
            && let Some(point_index) = index.get(&PointKey::from(point))
        {
            builders
                .entry(dsu.find(*point_index))
                .or_default()
                .pins
                .push(pin.clone());
        }
    }
    let mut nets = Vec::new();
    for (number, (_, builder)) in builders.into_iter().enumerate() {
        let labels = builder.labels.into_iter().collect::<Vec<_>>();
        let id = labels
            .first()
            .cloned()
            .unwrap_or_else(|| format!("N{}", number + 1));
        let has_no_connect = builder.no_connect_count > 0;
        if builder.dangling_endpoint_count > 0
            && builder.pins.is_empty()
            && labels.is_empty()
            && !has_no_connect
        {
            check.add(
                Finding::new(
                    "KICAD_FLOATING_WIRE_ENDPOINT",
                    Severity::Warning,
                    "wire endpoint does not connect to another wire, pin, label, or junction",
                )
                .with_detail("net", id.clone()),
            );
        }
        if builder.pins.len() == 1 && labels.is_empty() && !has_no_connect {
            check.add(
                Finding::new(
                    "KICAD_SINGLE_PIN_NET",
                    Severity::Warning,
                    "network contains only one identifiable pin and no label",
                )
                .with_detail("net", id.clone())
                .with_detail(
                    "reference",
                    builder.pins[0].reference.clone().unwrap_or_default(),
                )
                .with_detail("pin", builder.pins[0].number.clone()),
            );
        }
        let has_output = builder.pins.iter().any(|pin| {
            matches!(
                pin.electrical_type
                    .as_deref()
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("output") | Some("power_output") | Some("tristate")
            )
        });
        let has_input = builder.pins.iter().any(|pin| {
            matches!(
                pin.electrical_type
                    .as_deref()
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("input") | Some("power_input") | Some("bidirectional")
            )
        });
        if has_input && !has_output && !has_no_connect {
            check.add(
                Finding::new(
                    "KICAD_NO_DRIVER",
                    Severity::Warning,
                    "network has input-like pins but no identifiable output driver",
                )
                .with_detail("net", id.clone()),
            );
        }
        if has_output
            && builder
                .pins
                .iter()
                .filter(|pin| {
                    matches!(
                        pin.electrical_type
                            .as_deref()
                            .map(str::to_ascii_lowercase)
                            .as_deref(),
                        Some("output") | Some("power_output")
                    )
                })
                .count()
                > 1
        {
            check.add(
                Finding::new(
                    "KICAD_MULTIPLE_DRIVERS",
                    Severity::Error,
                    "network has multiple output-like pins",
                )
                .with_detail("net", id.clone()),
            );
        }
        nets.push(KicadConnectivityNet {
            id,
            labels,
            pins: builder.pins,
            wire_point_count: builder.wire_point_count,
            no_connect_count: builder.no_connect_count,
        });
    }
    if document.pins.is_empty() && document.wires.is_empty() {
        check.add(Finding::new(
            "KICAD_NO_CONNECTIVITY_DATA",
            Severity::Warning,
            "schematic has no globally mappable pins or wires; connectivity review is incomplete",
        ));
    }
    KicadConnectivity {
        wire_count: document.wires.len(),
        junction_count: document.junctions.len(),
        no_connect_count: document.no_connects.len(),
        pin_count: document.pins.len(),
        nets,
        check,
    }
}

#[derive(Default)]
struct ConnectivityBuilder {
    labels: BTreeSet<String>,
    pins: Vec<KicadPin>,
    wire_point_count: usize,
    dangling_endpoint_count: usize,
    junction_count: usize,
    no_connect_count: usize,
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

struct DisjointSet {
    parents: Vec<usize>,
    ranks: Vec<usize>,
}

impl DisjointSet {
    fn new(length: usize) -> Self {
        Self {
            parents: (0..length).collect(),
            ranks: vec![0; length],
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parents[value] != value {
            let parent = self.parents[value];
            self.parents[value] = self.find(parent);
        }
        self.parents[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let mut left = self.find(left);
        let mut right = self.find(right);
        if left == right {
            return;
        }
        if self.ranks[left] < self.ranks[right] {
            std::mem::swap(&mut left, &mut right);
        }
        self.parents[right] = left;
        if self.ranks[left] == self.ranks[right] {
            self.ranks[left] += 1;
        }
    }
}

pub fn inspect_kicad_project(
    documents: Vec<KicadDocument>,
    source: impl Into<String>,
) -> KicadProjectSnapshot {
    let source = source.into();
    let mut check = CheckResult::new("kicad_project", "summarized KiCad project documents");
    let mut references = BTreeMap::<String, KicadDocumentKind>::new();
    let mut nets = BTreeSet::new();
    let component_count = documents
        .iter()
        .map(|document| document.components.len())
        .sum();
    let label_count = documents.iter().map(|document| document.labels.len()).sum();
    let layer_count = documents.iter().map(|document| document.layers.len()).sum();
    for document in &documents {
        check.status = check.status.combine(document.check.status);
        check.findings.extend(document.check.findings.clone());
        for component in &document.components {
            let reference = component.reference.trim().to_ascii_uppercase();
            if !reference.is_empty()
                && references
                    .get(&reference)
                    .is_some_and(|kind| *kind == document.kind)
            {
                check.add(
                    Finding::new(
                        "KICAD_PROJECT_DUPLICATE_REFERENCE",
                        Severity::Error,
                        "reference is duplicated across KiCad documents",
                    )
                    .at_path(source.clone())
                    .with_detail("reference", reference),
                );
            } else if !reference.is_empty() {
                references.insert(reference.clone(), document.kind);
            }
        }
        for net in &document.nets {
            nets.insert(net.name.to_ascii_uppercase());
        }
    }
    if documents.is_empty() {
        check.add(
            Finding::new(
                "KICAD_PROJECT_EMPTY",
                Severity::Error,
                "no KiCad schematic or PCB document was found",
            )
            .at_path(source.clone()),
        );
    }
    let net_count = nets.len();
    KicadProjectSnapshot {
        source,
        documents,
        component_count,
        net_count,
        label_count,
        layer_count,
        check,
    }
}

pub fn compare_kicad_schematic_pcb(project: &KicadProjectSnapshot) -> KicadConsistencyReport {
    let schematic = project
        .documents
        .iter()
        .find(|document| document.kind == KicadDocumentKind::Schematic);
    let pcb = project
        .documents
        .iter()
        .find(|document| document.kind == KicadDocumentKind::Pcb);
    let mut check = CheckResult::new(
        "kicad_schematic_pcb_consistency",
        "compared schematic symbols to PCB footprints and pads",
    );
    let empty_report = |check: CheckResult| KicadConsistencyReport {
        schematic_source: schematic.map(|document| document.source.clone()),
        pcb_source: pcb.map(|document| document.source.clone()),
        missing_on_pcb: Vec::new(),
        missing_in_schematic: Vec::new(),
        pin_pad_mismatches: Vec::new(),
        footprint_drift: Vec::new(),
        net_drift: Vec::new(),
        check,
    };
    let Some(schematic) = schematic else {
        check.status = Status::Skipped;
        check.summary = "no schematic document was available for cross-check".to_owned();
        return empty_report(check);
    };
    let Some(pcb) = pcb else {
        check.status = Status::Skipped;
        check.summary = "no PCB document was available for cross-check".to_owned();
        return empty_report(check);
    };

    let schematic_components = component_map_for_document(schematic);
    let pcb_components = component_map_for_document(pcb);
    let missing_on_pcb = schematic_components
        .keys()
        .filter(|reference| !pcb_components.contains_key(*reference))
        .cloned()
        .collect::<Vec<_>>();
    let missing_in_schematic = pcb_components
        .keys()
        .filter(|reference| !schematic_components.contains_key(*reference))
        .cloned()
        .collect::<Vec<_>>();
    for reference in &missing_on_pcb {
        check.add(
            Finding::new(
                "KICAD_COMPONENT_MISSING_ON_PCB",
                Severity::Error,
                "schematic component is absent from PCB footprints",
            )
            .with_detail("reference", reference.clone()),
        );
    }
    for reference in &missing_in_schematic {
        check.add(
            Finding::new(
                "KICAD_COMPONENT_MISSING_IN_SCHEMATIC",
                Severity::Warning,
                "PCB footprint has no matching schematic component",
            )
            .with_detail("reference", reference.clone()),
        );
    }

    let mut footprint_drift = Vec::new();
    for reference in schematic_components
        .keys()
        .filter(|reference| pcb_components.contains_key(*reference))
    {
        let schematic_footprint = schematic_components[reference].footprint.as_deref();
        let pcb_footprint = pcb_components[reference].footprint.as_deref();
        if let (Some(schematic_footprint), Some(pcb_footprint)) =
            (schematic_footprint, pcb_footprint)
            && !schematic_footprint.eq_ignore_ascii_case(pcb_footprint)
        {
            footprint_drift.push(reference.clone());
            check.add(
                Finding::new(
                    "KICAD_FOOTPRINT_DRIFT",
                    Severity::Error,
                    "schematic footprint property differs from PCB footprint",
                )
                .with_detail("reference", reference.clone())
                .with_detail("schematic", schematic_footprint)
                .with_detail("pcb", pcb_footprint),
            );
        }
    }

    let connectivity = analyze_kicad_connectivity(schematic);
    let mut schematic_pin_nets = BTreeMap::<(String, String), String>::new();
    for net in &connectivity.nets {
        let Some(label) = net.labels.first() else {
            continue;
        };
        for pin in &net.pins {
            if let Some(reference) = &pin.reference {
                schematic_pin_nets.insert(
                    (
                        reference.to_ascii_uppercase(),
                        pin.number.to_ascii_uppercase(),
                    ),
                    label.clone(),
                );
            }
        }
    }
    let mut pin_pad_mismatches = Vec::new();
    let mut net_drift = Vec::new();
    for reference in schematic_components
        .keys()
        .filter(|reference| pcb_components.contains_key(*reference))
    {
        let schematic_pins = schematic
            .pins
            .iter()
            .filter(|pin| {
                pin.reference
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(reference))
            })
            .map(|pin| pin.number.to_ascii_uppercase())
            .collect::<BTreeSet<_>>();
        let pcb_pads = pcb_components[reference]
            .pads
            .iter()
            .map(|pad| pad.number.to_ascii_uppercase())
            .collect::<BTreeSet<_>>();
        for pin_number in schematic_pins.difference(&pcb_pads) {
            pin_pad_mismatches.push(KicadPinPadMismatch {
                reference: reference.clone(),
                pin_number: pin_number.clone(),
                schematic_net: None,
                pcb_net: None,
                kind: "missing_pad".to_owned(),
            });
            check.add(
                Finding::new(
                    "KICAD_PIN_MISSING_PAD",
                    Severity::Error,
                    "schematic pin has no matching PCB pad",
                )
                .with_detail("reference", reference.clone())
                .with_detail("pin", pin_number.clone()),
            );
        }
        for pad_number in pcb_pads.difference(&schematic_pins) {
            check.add(
                Finding::new(
                    "KICAD_PAD_MISSING_PIN",
                    Severity::Warning,
                    "PCB pad has no matching schematic pin",
                )
                .with_detail("reference", reference.clone())
                .with_detail("pad", pad_number.clone()),
            );
        }
        for pad in &pcb_components[reference].pads {
            let key = (
                reference.to_ascii_uppercase(),
                pad.number.to_ascii_uppercase(),
            );
            let Some(expected_net) = schematic_pin_nets.get(&key) else {
                continue;
            };
            match pad.net.as_deref() {
                None => {
                    net_drift.push(format!("{}:{}", reference, pad.number));
                    pin_pad_mismatches.push(KicadPinPadMismatch {
                        reference: reference.clone(),
                        pin_number: pad.number.clone(),
                        schematic_net: Some(expected_net.clone()),
                        pcb_net: None,
                        kind: "missing_pcb_net".to_owned(),
                    });
                    check.add(
                        Finding::new(
                            "KICAD_PAD_MISSING_NET",
                            Severity::Error,
                            "PCB pad has no net while schematic pin has a labelled net",
                        )
                        .with_detail("reference", reference.clone())
                        .with_detail("pad", pad.number.clone())
                        .with_detail("schematic_net", expected_net.clone()),
                    );
                }
                Some(actual_net) if !expected_net.eq_ignore_ascii_case(actual_net) => {
                    net_drift.push(format!("{}:{}", reference, pad.number));
                    pin_pad_mismatches.push(KicadPinPadMismatch {
                        reference: reference.clone(),
                        pin_number: pad.number.clone(),
                        schematic_net: Some(expected_net.clone()),
                        pcb_net: Some(actual_net.to_owned()),
                        kind: "net_mismatch".to_owned(),
                    });
                    check.add(
                        Finding::new(
                            "KICAD_PIN_PAD_NET_MISMATCH",
                            Severity::Error,
                            "schematic pin net differs from PCB pad net",
                        )
                        .with_detail("reference", reference.clone())
                        .with_detail("pad", pad.number.clone())
                        .with_detail("schematic_net", expected_net.clone())
                        .with_detail("pcb_net", actual_net),
                    );
                }
                Some(_) => {}
            }
        }
    }
    KicadConsistencyReport {
        schematic_source: Some(schematic.source.clone()),
        pcb_source: Some(pcb.source.clone()),
        missing_on_pcb,
        missing_in_schematic,
        pin_pad_mismatches,
        footprint_drift,
        net_drift,
        check,
    }
}

pub fn trace_kicad_signal(project: &KicadProjectSnapshot, query: &str) -> KicadSignalTrace {
    let query = query.trim().to_owned();
    let needle = query.to_ascii_lowercase();
    let mut components = Vec::new();
    let mut nets = Vec::new();
    let mut labels = BTreeSet::new();
    for document in &project.documents {
        components.extend(
            document
                .components
                .iter()
                .filter(|component| {
                    [
                        component.reference.as_str(),
                        component.value.as_deref().unwrap_or_default(),
                        component.footprint.as_deref().unwrap_or_default(),
                        component.library_id.as_deref().unwrap_or_default(),
                    ]
                    .iter()
                    .any(|value| value.to_ascii_lowercase().contains(&needle))
                })
                .cloned(),
        );
        nets.extend(
            document
                .nets
                .iter()
                .filter(|net| net.name.to_ascii_lowercase().contains(&needle))
                .cloned(),
        );
        labels.extend(
            document
                .labels
                .iter()
                .filter(|label| label.to_ascii_lowercase().contains(&needle))
                .cloned(),
        );
    }
    components.sort_by(|left, right| left.reference.cmp(&right.reference));
    nets.sort_by(|left, right| left.name.cmp(&right.name));
    let mut check = CheckResult::new("kicad_signal_trace", "traced matching KiCad objects");
    if components.is_empty() && nets.is_empty() && labels.is_empty() {
        check.add(
            Finding::new(
                "KICAD_SIGNAL_NOT_FOUND",
                Severity::Warning,
                "query did not match a component, net, or label",
            )
            .with_detail("query", query.clone()),
        );
    }
    KicadSignalTrace {
        query,
        matched_components: components,
        matched_nets: nets,
        matched_labels: labels.into_iter().collect(),
        check,
    }
}

pub fn review_kicad_power_tree(project: &KicadProjectSnapshot) -> KicadPowerTree {
    let mut power_nets = BTreeSet::new();
    let mut ground_nets = BTreeSet::new();
    let mut supply_components = Vec::new();
    for document in &project.documents {
        for net in &document.nets {
            if is_ground_name(&net.name) {
                ground_nets.insert(net.name.clone());
            } else if is_power_name(&net.name) {
                power_nets.insert(net.name.clone());
            }
        }
        for label in &document.labels {
            if is_ground_name(label) {
                ground_nets.insert(label.clone());
            } else if is_power_name(label) {
                power_nets.insert(label.clone());
            }
        }
        supply_components.extend(
            document
                .components
                .iter()
                .filter(|component| {
                    let value = component
                        .value
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let library = component
                        .library_id
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    value.contains("reg")
                        || value.contains("ldo")
                        || value.contains("converter")
                        || library.contains("regulator")
                })
                .cloned(),
        );
    }
    let mut check = CheckResult::new(
        "kicad_power_tree",
        "reviewed recognizable power and ground naming",
    );
    if ground_nets.is_empty() {
        check.add(Finding::new(
            "KICAD_POWER_NO_GROUND",
            Severity::Error,
            "no recognizable GND/ground net or label was found",
        ));
    }
    if power_nets.is_empty() {
        check.add(Finding::new(
            "KICAD_POWER_NO_SUPPLY",
            Severity::Warning,
            "no recognizable supply net or label was found",
        ));
    }
    KicadPowerTree {
        power_nets: power_nets.into_iter().collect(),
        ground_nets: ground_nets.into_iter().collect(),
        supply_components,
        check,
    }
}

pub fn compare_kicad_revisions(
    left: &KicadProjectSnapshot,
    right: &KicadProjectSnapshot,
) -> KicadRevisionDiff {
    let left_components = component_map(left);
    let right_components = component_map(right);
    let left_nets = net_set(left);
    let right_nets = net_set(right);
    let added_components = right_components
        .keys()
        .filter(|reference| !left_components.contains_key(*reference))
        .cloned()
        .collect::<Vec<_>>();
    let removed_components = left_components
        .keys()
        .filter(|reference| !right_components.contains_key(*reference))
        .cloned()
        .collect::<Vec<_>>();
    let changed_components = left_components
        .iter()
        .filter_map(|(reference, left)| {
            right_components.get(reference).and_then(|right| {
                (optional_changed(&left.value, &right.value)
                    || optional_changed(&left.footprint, &right.footprint))
                .then(|| reference.clone())
            })
        })
        .collect::<Vec<_>>();
    let added_nets: Vec<String> = right_nets.difference(&left_nets).cloned().collect();
    let removed_nets: Vec<String> = left_nets.difference(&right_nets).cloned().collect();
    let mut check = CheckResult::new(
        "kicad_revision_diff",
        "compared KiCad component and net revisions",
    );
    if !removed_components.is_empty() || !removed_nets.is_empty() {
        check.add(Finding::new(
            "KICAD_REVISION_REMOVALS",
            Severity::Warning,
            "revision comparison found removed components or nets; review ECO impact",
        ));
    }
    KicadRevisionDiff {
        left_source: left.source.clone(),
        right_source: right.source.clone(),
        added_components,
        removed_components,
        changed_components,
        added_nets,
        removed_nets,
        check,
    }
}

fn collect_document_data(
    node: &SExpr,
    kind: KicadDocumentKind,
    components: &mut Vec<KicadComponent>,
    nets: &mut Vec<KicadNet>,
    labels: &mut BTreeSet<String>,
    layers: &mut BTreeSet<String>,
) {
    let Some(items) = node.as_list() else { return };
    let head = atom(items.first());
    match (kind, head) {
        (KicadDocumentKind::Pcb, "footprint") => {
            if let Some(component) = parse_pcb_footprint(node) {
                components.push(component);
            }
        }
        (KicadDocumentKind::Pcb, "net") => {
            if let Some(net) = parse_net(node) {
                nets.push(net);
            }
        }
        (KicadDocumentKind::Pcb, "layers") => {
            for layer in items.iter().skip(1) {
                let Some(layer_items) = layer.as_list() else {
                    continue;
                };
                if let Some(name) = string_at(layer_items.get(1)) {
                    layers.insert(name.to_owned());
                }
            }
        }
        (KicadDocumentKind::Schematic, "symbol") => {
            if let Some(component) = parse_schematic_symbol(node) {
                components.push(component);
            }
        }
        (KicadDocumentKind::Schematic, "net") => {
            if let Some(net) = parse_net(node) {
                nets.push(net);
            }
        }
        (KicadDocumentKind::Schematic, "label" | "global_label" | "hierarchical_label") => {
            if let Some(label) = string_at(items.get(1)) {
                labels.insert(label.to_owned());
            }
        }
        _ => {}
    }
    for child in items.iter().skip(1) {
        collect_document_data(child, kind, components, nets, labels, layers);
    }
}

fn collect_pcb_geometry(root: &SExpr) -> (Vec<KicadWire>, Vec<KicadTrack>, Vec<KicadVia>) {
    let mut outline = Vec::new();
    let mut tracks = Vec::new();
    let mut vias = Vec::new();
    collect_pcb_geometry_recursive(root, &mut outline, &mut tracks, &mut vias);
    (outline, tracks, vias)
}

fn collect_pcb_geometry_recursive(
    node: &SExpr,
    outline: &mut Vec<KicadWire>,
    tracks: &mut Vec<KicadTrack>,
    vias: &mut Vec<KicadVia>,
) {
    let Some(items) = node.as_list() else { return };
    match atom(items.first()) {
        "gr_line" => {
            let layer = find_direct_string(node, "layer");
            if layer
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("Edge.Cuts"))
                && let (Some(start), Some(end)) = (
                    find_direct_point(node, "start"),
                    find_direct_point(node, "end"),
                )
            {
                outline.push(KicadWire { start, end });
            }
        }
        "segment" => {
            if let (Some(start), Some(end)) = (
                find_direct_point(node, "start"),
                find_direct_point(node, "end"),
            ) {
                tracks.push(KicadTrack {
                    start,
                    end,
                    width: find_direct_number(node, "width"),
                    layer: find_direct_string(node, "layer"),
                    net_code: find_direct_integer(node, "net"),
                });
            }
        }
        "via" => {
            if let Some(point) = find_direct_point(node, "at") {
                vias.push(KicadVia {
                    point,
                    size: find_direct_number(node, "size"),
                    drill: find_direct_number(node, "drill"),
                    layers: find_direct_strings(node, "layers"),
                    net_code: find_direct_integer(node, "net"),
                });
            }
        }
        _ => {}
    }
    for child in items.iter().skip(1) {
        collect_pcb_geometry_recursive(child, outline, tracks, vias);
    }
}

fn collect_schematic_connectivity(
    root: &SExpr,
    labels: &mut BTreeSet<String>,
    label_points: &mut Vec<KicadLabel>,
) -> (
    Vec<KicadWire>,
    Vec<KicadPoint>,
    Vec<KicadPoint>,
    Vec<KicadPin>,
) {
    let mut wires = Vec::new();
    let mut junctions = Vec::new();
    let mut no_connects = Vec::new();
    let mut pins = Vec::new();
    collect_connectivity_nodes(
        root,
        None,
        labels,
        label_points,
        &mut wires,
        &mut junctions,
        &mut no_connects,
        &mut pins,
    );
    enrich_library_pins(root, &mut pins);
    (wires, junctions, no_connects, pins)
}

#[allow(clippy::too_many_arguments)]
fn collect_connectivity_nodes(
    node: &SExpr,
    symbol_reference: Option<&str>,
    labels: &mut BTreeSet<String>,
    label_points: &mut Vec<KicadLabel>,
    wires: &mut Vec<KicadWire>,
    junctions: &mut Vec<KicadPoint>,
    no_connects: &mut Vec<KicadPoint>,
    pins: &mut Vec<KicadPin>,
) {
    let Some(items) = node.as_list() else { return };
    let head = atom(items.first());
    if head == "lib_symbols" {
        return;
    }
    let current_reference = if head == "symbol" {
        find_property(node, "Reference")
    } else {
        None
    };
    let reference = current_reference.as_deref().or(symbol_reference);
    match head {
        "wire" => {
            if let Some(wire) = parse_wire(node) {
                wires.push(wire);
            }
        }
        "junction" => {
            if let Some(point) = find_direct_point(node, "at") {
                junctions.push(point);
            }
        }
        "no_connect" => {
            if let Some(point) = find_direct_point(node, "at") {
                no_connects.push(point);
            }
        }
        "label" | "global_label" | "hierarchical_label" => {
            if let Some(name) = string_at(items.get(1))
                && let Some(point) = find_direct_point(node, "at")
            {
                labels.insert(name.to_owned());
                label_points.push(KicadLabel {
                    name: name.to_owned(),
                    point,
                });
            }
        }
        "pin" => {
            if let Some(pin) = parse_pin(node, reference) {
                pins.push(pin);
            }
        }
        _ => {}
    }
    for child in items.iter().skip(1) {
        collect_connectivity_nodes(
            child,
            reference,
            labels,
            label_points,
            wires,
            junctions,
            no_connects,
            pins,
        );
    }
}

fn parse_wire(node: &SExpr) -> Option<KicadWire> {
    let items = node.as_list()?;
    let points = items.iter().skip(1).find_map(|child| {
        let values = child.as_list()?;
        if atom(values.first()) != "pts" {
            return None;
        }
        let coordinates = values
            .iter()
            .skip(1)
            .filter_map(|point| {
                let point_items = point.as_list()?;
                (atom(point_items.first()) == "xy").then(|| parse_point(point_items))
            })
            .flatten()
            .collect::<Vec<_>>();
        (coordinates.len() >= 2).then(|| KicadWire {
            start: coordinates[0].clone(),
            end: coordinates[coordinates.len() - 1].clone(),
        })
    })?;
    Some(points)
}

fn parse_pin(node: &SExpr, reference: Option<&str>) -> Option<KicadPin> {
    let items = node.as_list()?;
    let electrical_type = atom_opt(items.get(1)).map(str::to_owned);
    let point = items.iter().skip(1).find_map(|child| {
        let values = child.as_list()?;
        (atom(values.first()) == "at")
            .then(|| parse_point(values))
            .flatten()
    });
    let name = find_direct_string(node, "name");
    let number = find_direct_string(node, "number")?;
    Some(KicadPin {
        reference: reference.map(str::to_owned),
        number,
        name,
        electrical_type,
        point,
        coordinate_source: "instance".to_owned(),
    })
}

fn find_direct_point(node: &SExpr, head: &str) -> Option<KicadPoint> {
    let items = node.as_list()?;
    items.iter().skip(1).find_map(|child| {
        let values = child.as_list()?;
        (atom(values.first()) == head)
            .then(|| parse_point(values))
            .flatten()
    })
}

fn parse_point(items: &[SExpr]) -> Option<KicadPoint> {
    Some(KicadPoint {
        x: atom_opt(items.get(1))?.parse().ok()?,
        y: atom_opt(items.get(2))?.parse().ok()?,
    })
}

fn parse_pcb_footprint(node: &SExpr) -> Option<KicadComponent> {
    let layer = find_direct_string(node, "layer");
    let (x, y) = parse_at(node);
    let reference = find_fp_reference(node).unwrap_or_default();
    let value = find_fp_text(node, "value");
    let footprint = node
        .as_list()
        .and_then(|items| string_at(items.get(1)))
        .map(str::to_owned);
    let pads = collect_pcb_pads(node);
    (!reference.is_empty()).then_some(KicadComponent {
        reference,
        value,
        footprint,
        library_id: None,
        x,
        y,
        layer,
        pads,
    })
}

fn parse_schematic_symbol(node: &SExpr) -> Option<KicadComponent> {
    let library_id = find_direct_string(node, "lib_id");
    library_id.as_ref()?;
    let reference = find_property(node, "Reference").or_else(|| find_property(node, "reference"));
    let value = find_property(node, "Value").or_else(|| find_property(node, "value"));
    let footprint = find_property(node, "Footprint").or_else(|| find_property(node, "footprint"));
    let (x, y) = parse_at(node);
    reference.map(|reference| KicadComponent {
        reference,
        value,
        footprint,
        library_id,
        x,
        y,
        layer: None,
        pads: Vec::new(),
    })
}

fn collect_pcb_pads(node: &SExpr) -> Vec<KicadPad> {
    let mut pads = Vec::new();
    collect_pcb_pads_recursive(node, parse_transform(node), &mut pads);
    pads
}

fn collect_pcb_pads_recursive(
    node: &SExpr,
    transform: SymbolTransform,
    output: &mut Vec<KicadPad>,
) {
    let Some(items) = node.as_list() else { return };
    if atom(items.first()) == "pad"
        && let Some(number) = string_at(items.get(1))
    {
        let point = find_direct_point(node, "at").map(|point| transform_point(&point, transform));
        let net = find_direct_net_name(node);
        output.push(KicadPad {
            number: number.to_owned(),
            net,
            x: point.as_ref().map(|point| point.x),
            y: point.as_ref().map(|point| point.y),
        });
    }
    for child in items.iter().skip(1) {
        collect_pcb_pads_recursive(child, transform, output);
    }
}

fn find_direct_net_name(node: &SExpr) -> Option<String> {
    let items = node.as_list()?;
    items.iter().skip(1).find_map(|child| {
        let values = child.as_list()?;
        (atom(values.first()) == "net")
            .then(|| string_at(values.get(2)).map(str::to_owned))
            .flatten()
    })
}

#[derive(Clone, Copy, Debug)]
struct SymbolTransform {
    x: f64,
    y: f64,
    rotation_degrees: f64,
    mirror_x: bool,
    mirror_y: bool,
}

fn enrich_library_pins(root: &SExpr, pins: &mut Vec<KicadPin>) {
    let libraries = collect_library_pin_definitions(root);
    let instances = collect_symbol_instances(root);
    for instance in instances {
        let Some(reference) = instance.reference.as_deref() else {
            continue;
        };
        let Some(library_id) = instance.library_id.as_deref() else {
            continue;
        };
        let Some(definitions) = libraries.get(library_id) else {
            continue;
        };
        let existing = pins
            .iter()
            .filter(|pin| pin.reference.as_deref() == Some(reference))
            .map(|pin| pin.number.to_ascii_uppercase())
            .collect::<BTreeSet<_>>();
        for definition in definitions {
            if existing.contains(&definition.number.to_ascii_uppercase()) {
                continue;
            }
            let Some(point) = &definition.point else {
                continue;
            };
            let point = transform_point(point, instance.transform);
            let mut pin = definition.clone();
            pin.reference = Some(reference.to_owned());
            pin.point = Some(point);
            pin.coordinate_source = "library_transform".to_owned();
            pins.push(pin);
        }
    }
}

#[derive(Clone, Debug)]
struct SymbolInstance {
    reference: Option<String>,
    library_id: Option<String>,
    transform: SymbolTransform,
}

fn collect_symbol_instances(root: &SExpr) -> Vec<SymbolInstance> {
    let mut instances = Vec::new();
    collect_symbol_instances_recursive(root, &mut instances);
    instances
}

fn collect_symbol_instances_recursive(node: &SExpr, output: &mut Vec<SymbolInstance>) {
    let Some(items) = node.as_list() else { return };
    let head = atom(items.first());
    if head == "lib_symbols" {
        return;
    }
    if head == "symbol" {
        let library_id = find_direct_string(node, "lib_id");
        if library_id.is_some() {
            output.push(SymbolInstance {
                reference: find_property(node, "Reference"),
                library_id,
                transform: parse_transform(node),
            });
        }
    }
    for child in items.iter().skip(1) {
        collect_symbol_instances_recursive(child, output);
    }
}

fn collect_library_pin_definitions(root: &SExpr) -> BTreeMap<String, Vec<KicadPin>> {
    let mut output = BTreeMap::new();
    let Some(items) = root.as_list() else {
        return output;
    };
    let Some(libraries) = items
        .iter()
        .skip(1)
        .find(|child| list_head(child) == Some("lib_symbols"))
    else {
        return output;
    };
    let Some(library_items) = libraries.as_list() else {
        return output;
    };
    for symbol in library_items.iter().skip(1) {
        let Some(symbol_items) = symbol.as_list() else {
            continue;
        };
        if atom(symbol_items.first()) != "symbol" {
            continue;
        }
        let Some(name) = string_at(symbol_items.get(1)) else {
            continue;
        };
        let mut pins = Vec::new();
        collect_library_pins_recursive(symbol, &mut pins);
        let mut seen = BTreeSet::new();
        pins.retain(|pin| seen.insert(pin.number.to_ascii_uppercase()));
        output.insert(name.to_owned(), pins);
    }
    output
}

fn collect_library_pins_recursive(node: &SExpr, output: &mut Vec<KicadPin>) {
    let Some(items) = node.as_list() else { return };
    if atom(items.first()) == "pin"
        && let Some(pin) = parse_pin(node, None)
    {
        output.push(pin);
    }
    for child in items.iter().skip(1) {
        collect_library_pins_recursive(child, output);
    }
}

fn parse_transform(node: &SExpr) -> SymbolTransform {
    let Some(items) = node.as_list() else {
        return SymbolTransform {
            x: 0.0,
            y: 0.0,
            rotation_degrees: 0.0,
            mirror_x: false,
            mirror_y: false,
        };
    };
    let mut transform = SymbolTransform {
        x: 0.0,
        y: 0.0,
        rotation_degrees: 0.0,
        mirror_x: false,
        mirror_y: false,
    };
    for child in items.iter().skip(1) {
        let Some(values) = child.as_list() else {
            continue;
        };
        match atom(values.first()) {
            "at" => {
                transform.x = atom_opt(values.get(1))
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0);
                transform.y = atom_opt(values.get(2))
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0);
                transform.rotation_degrees = atom_opt(values.get(3))
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0);
            }
            "mirror" => match atom_opt(values.get(1))
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str()
            {
                "x" => transform.mirror_x = true,
                "y" => transform.mirror_y = true,
                _ => {}
            },
            _ => {}
        }
    }
    transform
}

fn transform_point(point: &KicadPoint, transform: SymbolTransform) -> KicadPoint {
    let mut x = point.x;
    let mut y = point.y;
    if transform.mirror_x {
        x = -x;
    }
    if transform.mirror_y {
        y = -y;
    }
    let radians = transform.rotation_degrees.to_radians();
    KicadPoint {
        x: transform.x + (x * radians.cos() - y * radians.sin()),
        y: transform.y + (x * radians.sin() + y * radians.cos()),
    }
}

fn parse_net(node: &SExpr) -> Option<KicadNet> {
    let items = node.as_list()?;
    let code = atom(items.get(1)).parse::<i64>().ok();
    let name = string_at(items.get(2))
        .or_else(|| atom_opt(items.get(2)))
        .unwrap_or_default();
    (!name.is_empty()).then_some(KicadNet {
        code,
        name: name.to_owned(),
    })
}

fn find_fp_reference(node: &SExpr) -> Option<String> {
    find_fp_text(node, "reference").or_else(|| find_property(node, "Reference"))
}

fn find_fp_text(node: &SExpr, wanted: &str) -> Option<String> {
    let items = node.as_list()?;
    if atom(items.first()) == "fp_text" && atom(items.get(1)).eq_ignore_ascii_case(wanted) {
        return string_at(items.get(2)).map(str::to_owned);
    }
    for child in items.iter().skip(1) {
        if let Some(value) = find_fp_text(child, wanted) {
            return Some(value);
        }
    }
    None
}

fn find_property(node: &SExpr, wanted: &str) -> Option<String> {
    let items = node.as_list()?;
    if atom(items.first()) == "property" && atom(items.get(1)).eq_ignore_ascii_case(wanted) {
        return string_at(items.get(2)).map(str::to_owned);
    }
    for child in items.iter().skip(1) {
        if let Some(value) = find_property(child, wanted) {
            return Some(value);
        }
    }
    None
}

fn find_direct_string(node: &SExpr, head: &str) -> Option<String> {
    let items = node.as_list()?;
    items.iter().skip(1).find_map(|child| {
        let child_items = child.as_list()?;
        (atom(child_items.first()) == head)
            .then(|| string_at(child_items.get(1)).map(str::to_owned))
            .flatten()
    })
}

fn find_direct_number(node: &SExpr, head: &str) -> Option<f64> {
    find_direct_string(node, head).and_then(|value| value.parse().ok())
}

fn find_direct_integer(node: &SExpr, head: &str) -> Option<i64> {
    let items = node.as_list()?;
    items.iter().skip(1).find_map(|child| {
        let values = child.as_list()?;
        (atom(values.first()) == head)
            .then(|| atom_opt(values.get(1)).and_then(|value| value.parse().ok()))
            .flatten()
    })
}

fn find_direct_strings(node: &SExpr, head: &str) -> Vec<String> {
    let Some(items) = node.as_list() else {
        return Vec::new();
    };
    items
        .iter()
        .skip(1)
        .find_map(|child| {
            let values = child.as_list()?;
            if atom(values.first()) != head {
                return None;
            }
            Some(
                values
                    .iter()
                    .skip(1)
                    .filter_map(|value| string_at(Some(value)).map(str::to_owned))
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn find_direct_value(node: &SExpr, head: &str) -> Option<String> {
    find_direct_string(node, head)
}

fn parse_at(node: &SExpr) -> (Option<f64>, Option<f64>) {
    let Some(items) = node.as_list() else {
        return (None, None);
    };
    for child in items.iter().skip(1) {
        let Some(values) = child.as_list() else {
            continue;
        };
        if atom(values.first()) == "at" {
            return (
                atom_opt(values.get(1)).and_then(|value| value.parse().ok()),
                atom_opt(values.get(2)).and_then(|value| value.parse().ok()),
            );
        }
    }
    (None, None)
}

fn list_head(node: &SExpr) -> Option<&str> {
    node.as_list().and_then(|items| atom_opt(items.first()))
}

fn atom(node: Option<&SExpr>) -> &str {
    match node {
        Some(SExpr::Atom(value)) | Some(SExpr::String(value)) => value,
        _ => "",
    }
}

fn atom_opt(node: Option<&SExpr>) -> Option<&str> {
    match node {
        Some(SExpr::Atom(value)) | Some(SExpr::String(value)) => Some(value),
        _ => None,
    }
}

fn string_at(node: Option<&SExpr>) -> Option<&str> {
    match node {
        Some(SExpr::String(value)) => Some(value),
        Some(SExpr::Atom(value)) => Some(value),
        _ => None,
    }
}

fn component_map(project: &KicadProjectSnapshot) -> BTreeMap<String, KicadComponent> {
    let mut result = BTreeMap::new();
    for component in project
        .documents
        .iter()
        .flat_map(|document| document.components.iter())
    {
        let reference = component.reference.trim().to_ascii_uppercase();
        if reference.is_empty() {
            continue;
        }
        result
            .entry(reference)
            .and_modify(|existing: &mut KicadComponent| {
                if existing.value.is_none() && component.value.is_some() {
                    existing.value = component.value.clone();
                }
                if existing.footprint.is_none() && component.footprint.is_some() {
                    existing.footprint = component.footprint.clone();
                }
                if existing.library_id.is_none() && component.library_id.is_some() {
                    existing.library_id = component.library_id.clone();
                }
            })
            .or_insert_with(|| component.clone());
    }
    result
}

fn component_map_for_document(document: &KicadDocument) -> BTreeMap<String, KicadComponent> {
    document
        .components
        .iter()
        .filter_map(|component| {
            let reference = component.reference.trim().to_ascii_uppercase();
            (!reference.is_empty()).then_some((reference, component.clone()))
        })
        .collect()
}

fn optional_changed(left: &Option<String>, right: &Option<String>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left != right)
}

fn net_set(project: &KicadProjectSnapshot) -> BTreeSet<String> {
    project
        .documents
        .iter()
        .flat_map(|document| document.nets.iter())
        .map(|net| net.name.to_ascii_uppercase())
        .collect()
}

fn is_ground_name(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_uppercase().as_str(),
        "GND" | "AGND" | "DGND" | "PGND" | "GROUND" | "0"
    )
}

fn is_power_name(value: &str) -> bool {
    let upper = value.trim().to_ascii_uppercase();
    is_ground_name(&upper)
        || upper.starts_with("VCC")
        || upper.starts_with("VDD")
        || upper.starts_with("VSS")
        || upper.starts_with("VIN")
        || upper.starts_with("VOUT")
        || upper.starts_with('+')
        || upper.starts_with('-')
}

#[derive(Clone, Debug)]
enum SExpr {
    List(Vec<SExpr>),
    Atom(String),
    String(String),
}

impl SExpr {
    fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            Self::List(items) => Some(items),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
enum Token {
    Open,
    Close,
    Atom(String),
    String(String),
}

fn parse_sexpr(input: &str) -> Result<SExpr, String> {
    let tokens = tokenize(input)?;
    let mut index = 0;
    let expression = parse_expression(&tokens, &mut index)?;
    if index != tokens.len() {
        return Err("根表达式之后仍有内容".to_owned());
    }
    Ok(expression)
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            byte if byte.is_ascii_whitespace() => index += 1,
            b';' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'(' => {
                tokens.push(Token::Open);
                index += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                index += 1;
            }
            b'"' => {
                index += 1;
                let mut value = String::new();
                let mut closed = false;
                while index < bytes.len() {
                    let character = input[index..]
                        .chars()
                        .next()
                        .ok_or_else(|| "字符串包含无效 UTF-8".to_owned())?;
                    index += character.len_utf8();
                    match character {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' if index < bytes.len() => {
                            let escaped = input[index..]
                                .chars()
                                .next()
                                .ok_or_else(|| "转义字符后缺少内容".to_owned())?;
                            index += escaped.len_utf8();
                            value.push(escaped);
                        }
                        character => value.push(character),
                    }
                }
                if !closed {
                    return Err("字符串缺少结束引号".to_owned());
                }
                tokens.push(Token::String(value));
            }
            _ => {
                let start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && bytes[index] != b'('
                    && bytes[index] != b')'
                {
                    index += 1;
                }
                tokens.push(Token::Atom(input[start..index].to_owned()));
            }
        }
    }
    Ok(tokens)
}

fn parse_expression(tokens: &[Token], index: &mut usize) -> Result<SExpr, String> {
    match tokens.get(*index) {
        Some(Token::Open) => {
            *index += 1;
            let mut items = Vec::new();
            loop {
                match tokens.get(*index) {
                    Some(Token::Close) => {
                        *index += 1;
                        return Ok(SExpr::List(items));
                    }
                    Some(_) => items.push(parse_expression(tokens, index)?),
                    None => return Err("列表缺少结束括号".to_owned()),
                }
            }
        }
        Some(Token::Atom(value)) => {
            *index += 1;
            Ok(SExpr::Atom(value.clone()))
        }
        Some(Token::String(value)) => {
            *index += 1;
            Ok(SExpr::String(value.clone()))
        }
        Some(Token::Close) => Err("出现多余结束括号".to_owned()),
        None => Err("表达式为空".to_owned()),
    }
}
