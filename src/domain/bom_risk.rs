use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{CheckResult, Finding, Severity};
use super::{BomDocument, ManufacturingProfile, validate_bom};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomRiskReport {
    pub source: String,
    pub risk_count: usize,
    pub risks: Vec<BomRisk>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomRisk {
    pub references: Vec<String>,
    pub category: String,
    pub severity: Severity,
    pub message: String,
    pub evidence: Option<String>,
}

pub fn review_bom_risk(document: &BomDocument, profile: ManufacturingProfile) -> BomRiskReport {
    let base = validate_bom(document, profile);
    let mut check = CheckResult::new("bom_risk", "reviewed BOM lifecycle and sourcing evidence");
    check.findings.extend(base.check.findings.clone());
    check.status = base.check.status;
    let mut risks = Vec::new();

    for entry in &document.entries {
        if entry.do_not_place {
            continue;
        }
        let references = entry.references.clone();
        let evidence = entry.manufacturer_part_number.clone();
        if entry.manufacturer_part_number.is_some() && entry.manufacturer.is_none() {
            add_risk(
                &mut check,
                &mut risks,
                references.clone(),
                "identity",
                Severity::Warning,
                "MPN exists but manufacturer identity is not recorded",
                evidence.clone(),
            );
        }
        if entry.lifecycle_status.is_none() {
            add_risk(
                &mut check,
                &mut risks,
                references.clone(),
                "lifecycle",
                Severity::Warning,
                "component lifecycle status has no dated supplier evidence",
                evidence.clone(),
            );
        } else if let Some(status) = &entry.lifecycle_status {
            let normalized = status.to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "eol" | "obsolete" | "nrnd" | "not recommended"
            ) {
                add_risk(
                    &mut check,
                    &mut risks,
                    references.clone(),
                    "lifecycle",
                    if profile == ManufacturingProfile::Jlcpcb {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                    "component lifecycle status indicates EOL/obsolete/NRND risk",
                    Some(status.clone()),
                );
            }
        }
        if entry.supplier.is_none() {
            add_risk(
                &mut check,
                &mut risks,
                references.clone(),
                "supply",
                Severity::Warning,
                "no supplier or source-of-truth availability evidence is recorded",
                evidence.clone(),
            );
        }
        if entry.alternate_part_number.is_none() {
            add_risk(
                &mut check,
                &mut risks,
                references,
                "single_source",
                Severity::Info,
                "no approved alternate part is recorded; treat as a single-source candidate",
                evidence,
            );
        }
    }
    BomRiskReport {
        source: document.source.clone(),
        risk_count: risks.len(),
        risks,
        check,
    }
}

fn add_risk(
    check: &mut CheckResult,
    risks: &mut Vec<BomRisk>,
    references: Vec<String>,
    category: &str,
    severity: Severity,
    message: &str,
    evidence: Option<String>,
) {
    check.add(
        Finding::new(
            format!("BOM_RISK_{}", category.to_ascii_uppercase()),
            severity,
            message,
        )
        .with_detail("references", references.clone()),
    );
    risks.push(BomRisk {
        references,
        category: category.to_owned(),
        severity,
        message: message.to_owned(),
        evidence,
    });
}
