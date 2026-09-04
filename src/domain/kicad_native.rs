use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{CheckResult, DomainError, DomainResult, Finding, Severity};

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
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadNet {
    pub code: Option<i64>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadDocument {
    pub source: String,
    pub kind: KicadDocumentKind,
    pub version: Option<String>,
    pub components: Vec<KicadComponent>,
    pub nets: Vec<KicadNet>,
    pub labels: Vec<String>,
    pub layers: Vec<String>,
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
    let mut layers = BTreeSet::new();
    collect_document_data(
        &root,
        kind,
        &mut components,
        &mut nets,
        &mut labels,
        &mut layers,
    );
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
    if kind == KicadDocumentKind::Schematic && nets.is_empty() && labels.is_empty() {
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
        layers: layers.into_iter().collect(),
        check,
    })
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
        (KicadDocumentKind::Pcb, "layer") => {
            let name = string_at(items.get(2)).or_else(|| string_at(items.get(1)));
            if let Some(name) = name {
                layers.insert(name.to_owned());
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

fn parse_pcb_footprint(node: &SExpr) -> Option<KicadComponent> {
    let layer = find_direct_string(node, "layer");
    let (x, y) = parse_at(node);
    let reference = find_fp_reference(node).unwrap_or_default();
    let value = find_fp_text(node, "value");
    let footprint = node
        .as_list()
        .and_then(|items| string_at(items.get(1)))
        .map(str::to_owned);
    (!reference.is_empty()).then_some(KicadComponent {
        reference,
        value,
        footprint,
        library_id: None,
        x,
        y,
        layer,
    })
}

fn parse_schematic_symbol(node: &SExpr) -> Option<KicadComponent> {
    let library_id = find_direct_string(node, "lib_id");
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
    })
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
