use std::collections::{BTreeMap, BTreeSet};

use csv::{ReaderBuilder, StringRecord, Trim};
use encoding_rs::GB18030;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::profile::ManufacturingProfile;
use super::types::{CheckResult, DomainError, DomainResult, Finding, Severity, Status};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomDocument {
    pub source: String,
    pub encoding: String,
    pub headers: Vec<String>,
    pub entries: Vec<BomEntry>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomEntry {
    pub row: usize,
    pub references: Vec<String>,
    pub quantity: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity_value: Option<String>,
    pub value: Option<String>,
    pub footprint: Option<String>,
    pub manufacturer_part_number: Option<String>,
    pub do_not_place: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct CplDocument {
    pub source: String,
    pub encoding: String,
    pub headers: Vec<String>,
    pub placements: Vec<Placement>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Placement {
    pub row: usize,
    pub reference: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_value: Option<String>,
    pub layer: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomValidation {
    pub source: String,
    pub entry_count: usize,
    pub reference_count: usize,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct BomCplComparison {
    pub bom_source: String,
    pub cpl_source: String,
    pub bom_reference_count: usize,
    pub cpl_reference_count: usize,
    pub missing_from_cpl: Vec<String>,
    pub missing_from_bom: Vec<String>,
    pub check: CheckResult,
}

pub fn parse_bom(bytes: &[u8], source: impl Into<String>) -> DomainResult<BomDocument> {
    let source = source.into();
    let table = parse_csv(bytes, &source)?;
    let reference_column = required_column(&table, &REFERENCE_HEADERS, "BOM reference")?;
    let value_column = optional_column(&table, &VALUE_HEADERS);
    let quantity_column = optional_column(&table, &QUANTITY_HEADERS);
    let footprint_column = optional_column(&table, &FOOTPRINT_HEADERS);
    let mpn_column = optional_column(&table, &MPN_HEADERS);
    let dnp_column = optional_column(&table, &DNP_HEADERS);

    let mut entries = Vec::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        let references = split_references(value_at(row, reference_column));
        let quantity_value = quantity_column.and_then(|index| non_empty(value_at(row, index)));
        entries.push(BomEntry {
            row: row_index + 2,
            references,
            quantity: quantity_value
                .as_deref()
                .and_then(|value| value.parse::<usize>().ok()),
            quantity_value,
            value: value_column.and_then(|index| non_empty(value_at(row, index))),
            footprint: footprint_column.and_then(|index| non_empty(value_at(row, index))),
            manufacturer_part_number: mpn_column.and_then(|index| non_empty(value_at(row, index))),
            do_not_place: dnp_column
                .map(|index| {
                    let value = value_at(row, index);
                    if table.normalized_headers[index] == "populate" {
                        is_false(value)
                    } else {
                        is_dnp(value)
                    }
                })
                .unwrap_or(false),
        });
    }

    Ok(BomDocument {
        source,
        encoding: table.encoding,
        headers: table.headers,
        entries,
    })
}

pub fn parse_cpl(bytes: &[u8], source: impl Into<String>) -> DomainResult<CplDocument> {
    let source = source.into();
    let table = parse_csv(bytes, &source)?;
    let reference_column = required_column(&table, &REFERENCE_HEADERS, "CPL reference")?;
    let x_column = optional_column(&table, &X_HEADERS);
    let y_column = optional_column(&table, &Y_HEADERS);
    let rotation_column = optional_column(&table, &ROTATION_HEADERS);
    let layer_column = optional_column(&table, &LAYER_HEADERS);

    let mut placements = Vec::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        let reference = value_at(row, reference_column).trim().to_owned();
        placements.push(Placement {
            row: row_index + 2,
            reference,
            x: x_column.and_then(|index| parse_number(value_at(row, index))),
            y: y_column.and_then(|index| parse_number(value_at(row, index))),
            rotation: rotation_column.and_then(|index| parse_number(value_at(row, index))),
            rotation_value: rotation_column.and_then(|index| non_empty(value_at(row, index))),
            layer: layer_column.and_then(|index| non_empty(value_at(row, index))),
        });
    }

    Ok(CplDocument {
        source,
        encoding: table.encoding,
        headers: table.headers,
        placements,
    })
}

pub fn validate_bom(document: &BomDocument, profile: ManufacturingProfile) -> BomValidation {
    let rules = profile.rules();
    let mut check = CheckResult::new(
        "bom",
        format!("validated {} BOM rows", document.entries.len()),
    );
    let mut seen = BTreeMap::<String, usize>::new();
    let mut reference_count = 0;

    for entry in &document.entries {
        if entry.references.is_empty() {
            check.add(
                Finding::new(
                    "BOM_EMPTY_REFERENCE",
                    Severity::Error,
                    "BOM row has no designator",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            );
        }
        if rules.require_bom_value && entry.value.is_none() {
            check.add(
                Finding::new(
                    "BOM_MISSING_VALUE",
                    Severity::Error,
                    "BOM row has no value or comment",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            );
        }
        if rules.require_bom_footprint && entry.footprint.is_none() {
            check.add(
                Finding::new(
                    "BOM_MISSING_FOOTPRINT",
                    Severity::Error,
                    "BOM row has no footprint",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            );
        }
        if !entry.do_not_place && entry.manufacturer_part_number.is_none() {
            let severity = if profile == ManufacturingProfile::Jlcpcb {
                Severity::Error
            } else {
                Severity::Warning
            };
            check.add(
                Finding::new(
                    "BOM_MISSING_MPN",
                    severity,
                    "placed BOM row has no MPN or LCSC part number",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            );
        }
        match (&entry.quantity_value, entry.quantity) {
            (Some(_), None) => check.add(
                Finding::new(
                    "BOM_INVALID_QUANTITY",
                    Severity::Error,
                    "BOM quantity must be a non-negative integer",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            ),
            (Some(_), Some(quantity)) if quantity != entry.references.len() => check.add(
                Finding::new(
                    "BOM_QUANTITY_MISMATCH",
                    Severity::Error,
                    "BOM quantity does not match the number of designators",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row)
                .with_detail("quantity", quantity)
                .with_detail("reference_count", entry.references.len()),
            ),
            (None, _) if profile == ManufacturingProfile::Jlcpcb => check.add(
                Finding::new(
                    "BOM_MISSING_QUANTITY",
                    Severity::Error,
                    "JLCPCB BOM row has no Quantity column value",
                )
                .at_path(document.source.clone())
                .with_detail("row", entry.row),
            ),
            _ => {}
        }
        for reference in &entry.references {
            reference_count += 1;
            let normalized = reference.to_ascii_uppercase();
            if let Some(previous_row) = seen.insert(normalized, entry.row) {
                check.add(
                    Finding::new(
                        "BOM_DUPLICATE_REFERENCE",
                        Severity::Error,
                        "designator occurs in more than one BOM row",
                    )
                    .at_path(document.source.clone())
                    .with_detail("reference", reference.clone())
                    .with_detail("row", entry.row)
                    .with_detail("previous_row", previous_row),
                );
            }
        }
    }

    BomValidation {
        source: document.source.clone(),
        entry_count: document.entries.len(),
        reference_count,
        check,
    }
}

pub fn compare_bom_cpl(
    bom: &BomDocument,
    cpl: &CplDocument,
    profile: ManufacturingProfile,
) -> BomCplComparison {
    let rules = profile.rules();
    let mut check = CheckResult::new("bom_cpl", "compared BOM designators to placement rows");
    let mut bom_references = BTreeSet::new();
    let mut cpl_references = BTreeSet::new();

    for entry in &bom.entries {
        if !entry.do_not_place {
            for reference in &entry.references {
                bom_references.insert(reference.to_ascii_uppercase());
            }
        }
    }
    for placement in &cpl.placements {
        let reference = placement.reference.trim();
        if reference.is_empty() {
            check.add(
                Finding::new(
                    "CPL_EMPTY_REFERENCE",
                    Severity::Error,
                    "placement row has no designator",
                )
                .at_path(cpl.source.clone())
                .with_detail("row", placement.row),
            );
            continue;
        }
        if !cpl_references.insert(reference.to_ascii_uppercase()) {
            check.add(
                Finding::new(
                    "CPL_DUPLICATE_REFERENCE",
                    Severity::Error,
                    "designator occurs in more than one CPL row",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("row", placement.row),
            );
        }
        if rules.require_cpl_coordinates && (placement.x.is_none() || placement.y.is_none()) {
            check.add(
                Finding::new(
                    "CPL_MISSING_COORDINATE",
                    Severity::Error,
                    "placement has no valid X and Y coordinates",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("row", placement.row),
            );
        }
        match (&placement.rotation_value, placement.rotation) {
            (None, _) => check.add(
                Finding::new(
                    "CPL_MISSING_ROTATION",
                    Severity::Error,
                    "placement has no rotation",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("row", placement.row),
            ),
            (Some(_), None) => check.add(
                Finding::new(
                    "CPL_INVALID_ROTATION",
                    Severity::Error,
                    "placement rotation is not a finite number",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("row", placement.row),
            ),
            (Some(_), Some(_)) => {}
        }
        if rules.require_cpl_coordinates && placement.layer.is_none() {
            check.add(
                Finding::new(
                    "CPL_MISSING_LAYER",
                    Severity::Warning,
                    "placement has no assembly layer",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("row", placement.row),
            );
        }
        if let Some(layer) = &placement.layer
            && !is_top_or_bottom(layer)
        {
            check.add(
                Finding::new(
                    "CPL_INVALID_LAYER",
                    Severity::Error,
                    "placement layer must be Top or Bottom",
                )
                .at_path(cpl.source.clone())
                .with_detail("reference", placement.reference.clone())
                .with_detail("layer", layer.clone())
                .with_detail("row", placement.row),
            );
        }
    }

    let missing_from_cpl = bom_references
        .difference(&cpl_references)
        .cloned()
        .collect::<Vec<_>>();
    let missing_from_bom = cpl_references
        .difference(&bom_references)
        .cloned()
        .collect::<Vec<_>>();
    for reference in &missing_from_cpl {
        check.add(
            Finding::new(
                "CPL_MISSING_REFERENCE",
                Severity::Error,
                "BOM designator has no placement row",
            )
            .at_path(cpl.source.clone())
            .with_detail("reference", reference.clone()),
        );
    }
    for reference in &missing_from_bom {
        check.add(
            Finding::new(
                "CPL_UNKNOWN_REFERENCE",
                Severity::Error,
                "CPL designator is absent from BOM",
            )
            .at_path(cpl.source.clone())
            .with_detail("reference", reference.clone()),
        );
    }
    if cpl.placements.is_empty() {
        check.status = Status::Fail;
        check.add(
            Finding::new(
                "CPL_EMPTY",
                Severity::Error,
                "CPL contains no placement rows",
            )
            .at_path(cpl.source.clone()),
        );
    }

    BomCplComparison {
        bom_source: bom.source.clone(),
        cpl_source: cpl.source.clone(),
        bom_reference_count: bom_references.len(),
        cpl_reference_count: cpl_references.len(),
        missing_from_cpl,
        missing_from_bom,
        check,
    }
}

const REFERENCE_HEADERS: [&str; 6] = [
    "designator",
    "designators",
    "reference",
    "references",
    "ref",
    "refs",
];
const VALUE_HEADERS: [&str; 3] = ["value", "comment", "description"];
const QUANTITY_HEADERS: [&str; 2] = ["quantity", "qty"];
const FOOTPRINT_HEADERS: [&str; 4] = ["footprint", "package", "landpattern", "pattern"];
const MPN_HEADERS: [&str; 5] = [
    "mpn",
    "manufacturerpartnumber",
    "lcscpart",
    "lcscpartnumber",
    "partnumber",
];
const DNP_HEADERS: [&str; 5] = ["dnp", "donotplace", "notfitted", "notmounted", "populate"];
const X_HEADERS: [&str; 5] = ["midx", "x", "posx", "positionx", "centerx"];
const Y_HEADERS: [&str; 5] = ["midy", "y", "posy", "positiony", "centery"];
const ROTATION_HEADERS: [&str; 4] = ["rotation", "rotationdeg", "rot", "angle"];
const LAYER_HEADERS: [&str; 4] = ["layer", "side", "mountside", "assemblylayer"];

struct CsvTable {
    encoding: String,
    headers: Vec<String>,
    normalized_headers: Vec<String>,
    rows: Vec<StringRecord>,
}

fn parse_csv(bytes: &[u8], source: &str) -> DomainResult<CsvTable> {
    let (text, encoding) = decode_csv(bytes);
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()?
        .iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err(DomainError::InvalidInput(format!(
            "CSV has no headers: {source}"
        )));
    }
    let rows = reader.records().collect::<Result<Vec<_>, _>>()?;
    Ok(CsvTable {
        encoding,
        normalized_headers: headers
            .iter()
            .map(|header| normalize_header(header))
            .collect(),
        headers,
        rows,
    })
}

fn decode_csv(bytes: &[u8]) -> (String, String) {
    let utf8 = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(utf8) {
        return (
            text.to_owned(),
            if utf8.len() == bytes.len() {
                "utf-8"
            } else {
                "utf-8-bom"
            }
            .to_owned(),
        );
    }
    let (decoded, _, malformed) = GB18030.decode(bytes);
    let encoding = if malformed {
        "gb18030-with-replacement"
    } else {
        "gb18030"
    };
    (decoded.into_owned(), encoding.to_owned())
}

fn required_column(table: &CsvTable, aliases: &[&str], label: &str) -> DomainResult<usize> {
    optional_column(table, aliases).ok_or_else(|| {
        DomainError::InvalidInput(format!(
            "{label} column is missing; headers: {}",
            table.headers.join(", ")
        ))
    })
}

fn optional_column(table: &CsvTable, aliases: &[&str]) -> Option<usize> {
    table
        .normalized_headers
        .iter()
        .position(|header| aliases.iter().any(|alias| header == alias))
}

fn value_at(row: &StringRecord, index: usize) -> &str {
    row.get(index).unwrap_or_default()
}

fn normalize_header(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn split_references(value: &str) -> Vec<String> {
    value
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|reference| !reference.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_dnp(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "dnp" | "dnf" | "do not place" | "not fitted" | "no fit"
    )
}

fn is_false(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "n" | "not populated" | "not fitted"
    )
}

fn parse_number(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn is_top_or_bottom(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "top" | "bottom" | "t" | "b" | "front" | "back"
    )
}
