use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::kicad_native::{
    KicadDocumentKind, KicadProjectSnapshot, analyze_kicad_connectivity, review_kicad_power_tree,
};
use super::types::{CheckResult, Finding, Severity, Status};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct KicadDesignReview {
    pub source: String,
    pub status: Status,
    pub checks: Vec<CheckResult>,
    pub metrics: BTreeMap<String, usize>,
    pub findings: Vec<Finding>,
}

pub fn review_kicad_design(project: &KicadProjectSnapshot) -> KicadDesignReview {
    let mut checks = vec![project.check.clone()];
    let power = review_kicad_power_tree(project);
    checks.push(power.check.clone());
    let mut metrics = BTreeMap::from([
        ("components".to_owned(), project.component_count),
        ("nets".to_owned(), project.net_count),
        ("labels".to_owned(), project.label_count),
        ("layers".to_owned(), project.layer_count),
        ("power_nets".to_owned(), power.power_nets.len()),
        ("ground_nets".to_owned(), power.ground_nets.len()),
    ]);
    for document in &project.documents {
        if document.kind == KicadDocumentKind::Schematic {
            let connectivity = analyze_kicad_connectivity(document);
            metrics.insert("schematic_wires".to_owned(), connectivity.wire_count);
            metrics.insert("schematic_pins".to_owned(), connectivity.pin_count);
            metrics.insert(
                "schematic_inferred_nets".to_owned(),
                connectivity.nets.len(),
            );
            checks.push(connectivity.check);
        }
    }
    checks.push(review_component_contract(project));
    checks.push(review_clock_reset(project));
    checks.push(review_interfaces(project));
    checks.push(review_power_evidence(project));
    let status = checks
        .iter()
        .fold(Status::Pass, |status, check| status.combine(check.status));
    let findings = checks
        .iter()
        .flat_map(|check| check.findings.iter().cloned())
        .collect::<Vec<_>>();
    KicadDesignReview {
        source: project.source.clone(),
        status,
        checks,
        metrics,
        findings,
    }
}

fn review_component_contract(project: &KicadProjectSnapshot) -> CheckResult {
    let mut check = CheckResult::new(
        "kicad_component_contract",
        "reviewed component identity, value, footprint, and pad evidence",
    );
    for document in &project.documents {
        for component in &document.components {
            if component.reference.trim().is_empty() || component.reference.contains('?') {
                continue;
            }
            if document.kind == KicadDocumentKind::Schematic && component.footprint.is_none() {
                check.add(
                    Finding::new(
                        "KICAD_SCHEMATIC_MISSING_FOOTPRINT",
                        Severity::Warning,
                        "schematic component has no footprint property",
                    )
                    .at_path(document.source.clone())
                    .with_detail("reference", component.reference.clone()),
                );
            }
            if document.kind == KicadDocumentKind::Pcb && component.pads.is_empty() {
                check.add(
                    Finding::new(
                        "KICAD_PCB_NO_PADS",
                        Severity::Warning,
                        "PCB footprint has no recognizable pads",
                    )
                    .at_path(document.source.clone())
                    .with_detail("reference", component.reference.clone()),
                );
            }
        }
    }
    check
}

fn review_clock_reset(project: &KicadProjectSnapshot) -> CheckResult {
    let mut check = CheckResult::new(
        "kicad_clock_reset",
        "reviewed clock and reset naming evidence",
    );
    let labels = project
        .documents
        .iter()
        .flat_map(|document| document.labels.iter())
        .map(|label| label.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    let clock_labels = labels
        .iter()
        .filter(|label| is_clock_label(label))
        .cloned()
        .collect::<Vec<_>>();
    let reset_labels = labels
        .iter()
        .filter(|label| is_reset_label(label))
        .cloned()
        .collect::<Vec<_>>();
    let clock_components = project
        .documents
        .iter()
        .flat_map(|document| document.components.iter())
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
            [value.as_str(), library.as_str()].iter().any(|text| {
                text.contains("crystal")
                    || text.contains("oscillator")
                    || text.contains("clock")
                    || text.contains("xtal")
            })
        })
        .count();
    if clock_components > 0 && clock_labels.is_empty() {
        check.add(Finding::new(
            "KICAD_CLOCK_NO_LABEL_EVIDENCE",
            Severity::Warning,
            "clock-like components exist but no clock label/net evidence was found",
        ));
    }
    if !clock_labels.is_empty() && reset_labels.is_empty() {
        check.add(Finding::new(
            "KICAD_RESET_NO_LABEL_EVIDENCE",
            Severity::Warning,
            "clock evidence exists but no reset/NRST label was found; verify startup behavior",
        ));
    }
    if clock_labels.len() > 1 {
        check.add(
            Finding::new(
                "KICAD_MULTIPLE_CLOCK_DOMAINS",
                Severity::Info,
                "multiple clock-like labels were found; verify domain crossing and return paths",
            )
            .with_detail("labels", clock_labels.clone()),
        );
    }
    if !reset_labels.is_empty() {
        check.summary = format!(
            "reviewed {} clock labels and {} reset labels",
            clock_labels.len(),
            reset_labels.len()
        );
    }
    check
}

fn review_interfaces(project: &KicadProjectSnapshot) -> CheckResult {
    let mut check = CheckResult::new(
        "kicad_interfaces",
        "reviewed common interface naming completeness",
    );
    let labels = project
        .documents
        .iter()
        .flat_map(|document| document.labels.iter())
        .map(|label| label.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    let interfaces = [
        ("i2c", &["SDA", "SCL"] as &[&str]),
        ("spi", &["MOSI", "MISO", "SCK", "SCLK", "CS"]),
        ("uart", &["TX", "RX"]),
        ("can", &["CANH", "CANL"]),
        ("rs485", &["A", "B"]),
        ("usb", &["D+", "D-", "USB_DP", "USB_DM"]),
    ];
    for (interface, expected) in interfaces {
        let detected = labels
            .iter()
            .filter(|label| label.contains(&interface.to_ascii_uppercase()))
            .count();
        if detected == 0 {
            continue;
        }
        let present = expected
            .iter()
            .filter(|signal| {
                labels
                    .iter()
                    .any(|label| label == **signal || label.contains(*signal))
            })
            .count();
        let required = if interface == "spi" {
            4
        } else {
            expected.len()
        };
        if present < required {
            check.add(
                Finding::new(
                    "KICAD_INTERFACE_INCOMPLETE",
                    Severity::Warning,
                    "interface naming suggests a protocol but required signals are incomplete",
                )
                .with_detail("interface", interface)
                .with_detail("present_signals", present)
                .with_detail("required_signals", required),
            );
        }
    }
    check
}

fn review_power_evidence(project: &KicadProjectSnapshot) -> CheckResult {
    let power = review_kicad_power_tree(project);
    let mut check = CheckResult::new(
        "kicad_power_evidence",
        "reviewed supply labels against supply-component evidence",
    );
    if !power.power_nets.is_empty() && power.supply_components.is_empty() {
        check.add(Finding::new(
            "KICAD_POWER_NO_SUPPLY_COMPONENT",
            Severity::Warning,
            "power nets are named but no regulator/converter-like component was identified",
        ));
    }
    if !power.supply_components.is_empty() && power.power_nets.is_empty() {
        check.add(Finding::new(
            "KICAD_SUPPLY_COMPONENT_NO_NET",
            Severity::Warning,
            "regulator/converter-like component exists but no named power net was identified",
        ));
    }
    check
}

fn is_clock_label(label: &str) -> bool {
    label.contains("CLK")
        || label.contains("CLOCK")
        || label.contains("MCLK")
        || label.contains("SCLK")
        || label.contains("XTAL")
        || label.contains("OSC")
}

fn is_reset_label(label: &str) -> bool {
    label == "RESET"
        || label.contains("NRST")
        || label.ends_with("_RST")
        || label.starts_with("RST")
        || label.contains("POR")
}
