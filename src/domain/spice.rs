use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{CheckResult, Finding, Severity};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct SpiceValidation {
    pub source: String,
    pub component_count: usize,
    pub node_count: usize,
    pub analyses: Vec<String>,
    pub model_files: Vec<String>,
    pub components: Vec<SpiceComponent>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct SpiceComponent {
    pub line: usize,
    pub reference: String,
    pub kind: String,
    pub nodes: Vec<String>,
    pub value: Option<String>,
}

pub fn validate_spice_netlist(bytes: &[u8], source: impl Into<String>) -> SpiceValidation {
    let source = source.into();
    let text = String::from_utf8_lossy(bytes);
    let mut check = CheckResult::new("spice_netlist", "validated SPICE netlist structure");
    let mut components = Vec::new();
    let mut nodes = BTreeMap::<String, usize>::new();
    let mut references = BTreeSet::new();
    let mut analyses = Vec::new();
    let mut model_files = Vec::new();
    let mut ended = false;
    let mut saw_title = false;

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('*') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('$') {
            continue;
        }
        if line.starts_with('.') {
            let mut tokens = line.split_whitespace();
            let directive = tokens.next().unwrap_or_default().to_ascii_lowercase();
            match directive.as_str() {
                ".end" => ended = true,
                ".tran" | ".ac" | ".dc" | ".op" | ".noise" | ".tf" | ".pz" | ".sens" => {
                    analyses.push(directive.trim_start_matches('.').to_owned());
                }
                ".include" | ".lib" => {
                    if let Some(path) = tokens.next() {
                        model_files.push(path.trim_matches(['\"', '\'']).to_owned());
                    } else {
                        check.add(
                            Finding::new(
                                "SPICE_MISSING_MODEL_PATH",
                                Severity::Error,
                                "SPICE include/lib directive has no file path",
                            )
                            .at_path(source.clone())
                            .with_detail("line", line_number),
                        );
                    }
                }
                ".endc" | ".control" | ".param" | ".options" | ".model" | ".subckt" | ".ends" => {}
                _ => check.add(
                    Finding::new(
                        "SPICE_UNKNOWN_DIRECTIVE",
                        Severity::Warning,
                        "SPICE netlist contains an unrecognized directive",
                    )
                    .at_path(source.clone())
                    .with_detail("line", line_number)
                    .with_detail("directive", directive),
                ),
            }
            continue;
        }

        let tokens = line.split_whitespace().collect::<Vec<_>>();
        let reference = tokens.first().copied().unwrap_or_default();
        let Some(kind) = reference
            .chars()
            .next()
            .map(|character| character.to_ascii_uppercase())
        else {
            continue;
        };
        let reference_has_index = reference.len() > 1
            && reference[1..]
                .chars()
                .any(|character| character.is_ascii_digit());
        if !reference_has_index
            || !matches!(
                kind,
                'R' | 'C' | 'L' | 'D' | 'Q' | 'M' | 'J' | 'V' | 'I' | 'X' | 'T' | 'U'
            )
        {
            if !saw_title {
                saw_title = true;
                continue;
            }
            check.add(
                Finding::new(
                    "SPICE_UNKNOWN_COMPONENT",
                    Severity::Warning,
                    "line is neither a recognized SPICE component nor a directive",
                )
                .at_path(source.clone())
                .with_detail("line", line_number)
                .with_detail("token", reference),
            );
            continue;
        }
        saw_title = true;
        if !references.insert(reference.to_ascii_uppercase()) {
            check.add(
                Finding::new(
                    "SPICE_DUPLICATE_REFERENCE",
                    Severity::Error,
                    "SPICE component reference is duplicated",
                )
                .at_path(source.clone())
                .with_detail("line", line_number)
                .with_detail("reference", reference),
            );
        }
        let minimum_nodes = match kind {
            'R' | 'C' | 'L' | 'D' | 'V' | 'I' => 2,
            'Q' => 3,
            'M' => 4,
            'X' | 'T' | 'U' => 2,
            _ => 2,
        };
        let node_count = tokens.len().saturating_sub(2);
        if node_count < minimum_nodes {
            check.add(
                Finding::new(
                    "SPICE_TOO_FEW_NODES",
                    Severity::Error,
                    "SPICE component has too few node tokens",
                )
                .at_path(source.clone())
                .with_detail("line", line_number)
                .with_detail("reference", reference)
                .with_detail("expected_minimum", minimum_nodes)
                .with_detail("found", node_count),
            );
        }
        let nodes_for_component = tokens
            .iter()
            .skip(1)
            .take(minimum_nodes)
            .map(|node| (*node).to_owned())
            .collect::<Vec<_>>();
        for node in &nodes_for_component {
            *nodes.entry(node.to_ascii_uppercase()).or_default() += 1;
        }
        components.push(SpiceComponent {
            line: line_number,
            reference: reference.to_owned(),
            kind: kind.to_string(),
            nodes: nodes_for_component,
            value: tokens.last().map(|value| (*value).to_owned()),
        });
    }

    if !ended {
        check.add(
            Finding::new(
                "SPICE_MISSING_END",
                Severity::Error,
                "SPICE netlist has no .end directive",
            )
            .at_path(source.clone()),
        );
    }
    let has_ground = nodes.keys().any(|node| node == "0" || node == "GND");
    if !has_ground {
        check.add(
            Finding::new(
                "SPICE_MISSING_GROUND",
                Severity::Error,
                "SPICE netlist has no reference ground node 0/GND",
            )
            .at_path(source.clone()),
        );
    }
    for (node, count) in &nodes {
        if node != "0" && node != "GND" && *count == 1 {
            check.add(
                Finding::new(
                    "SPICE_SINGLE_USE_NODE",
                    Severity::Warning,
                    "SPICE node occurs on only one component pin",
                )
                .at_path(source.clone())
                .with_detail("node", node.clone()),
            );
        }
    }
    if analyses.is_empty() {
        check.add(
            Finding::new(
                "SPICE_NO_ANALYSIS",
                Severity::Warning,
                "SPICE netlist declares no analysis directive",
            )
            .at_path(source.clone()),
        );
    }
    SpiceValidation {
        source,
        component_count: components.len(),
        node_count: nodes.len(),
        analyses,
        model_files,
        components,
        check,
    }
}
