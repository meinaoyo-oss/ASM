use std::collections::{BTreeMap, BTreeSet};

use encoding_rs::GB18030;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{CheckResult, DomainError, DomainResult, Finding, Severity};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct RequirementDocument {
    pub source: String,
    pub format: String,
    pub encoding: String,
    pub sha256: String,
    pub requirements: Vec<Requirement>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Requirement {
    pub id: String,
    pub title: Option<String>,
    pub statement: String,
    pub status: Option<String>,
    pub verification_method: Option<String>,
    pub tags: Vec<String>,
    pub targets: Vec<String>,
    pub source_line: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct TraceLinkDocument {
    pub source: String,
    pub format: String,
    pub encoding: String,
    pub sha256: String,
    pub links: Vec<TraceLink>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct TraceLink {
    pub requirement_id: String,
    pub target: String,
    pub relation: String,
    pub evidence: Option<String>,
    pub source_line: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct RequirementQuality {
    pub source: String,
    pub requirement_count: usize,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct TraceabilityMatrix {
    pub requirements_source: String,
    pub links_source: Option<String>,
    pub requirement_count: usize,
    pub link_count: usize,
    pub covered_requirement_ids: Vec<String>,
    pub uncovered_requirement_ids: Vec<String>,
    pub invalid_requirement_ids: Vec<String>,
    pub trace_targets: Vec<String>,
    pub check: CheckResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ImpactAnalysis {
    pub requirement: Requirement,
    pub linked_targets: Vec<TraceLink>,
    pub related_requirement_ids: Vec<String>,
    pub check: CheckResult,
}

pub fn parse_requirements(
    bytes: &[u8],
    source: impl Into<String>,
) -> DomainResult<RequirementDocument> {
    let source = source.into();
    let (text, encoding) = decode_text(bytes);
    let sha256 = hash_bytes(bytes);
    let extension = source.rsplit('/').next().and_then(|name| {
        name.rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
    });

    let (format, requirements) = match extension.as_deref() {
        Some("json") => ("json".to_owned(), parse_requirements_json(&text)?),
        Some("csv") | Some("tsv") => ("csv".to_owned(), parse_requirements_csv(text.as_bytes())?),
        _ => ("markdown".to_owned(), parse_requirements_markdown(&text)),
    };
    Ok(RequirementDocument {
        source,
        format,
        encoding,
        sha256,
        requirements,
    })
}

pub fn parse_trace_links(
    bytes: &[u8],
    source: impl Into<String>,
) -> DomainResult<TraceLinkDocument> {
    let source = source.into();
    let (text, encoding) = decode_text(bytes);
    let sha256 = hash_bytes(bytes);
    let extension = source.rsplit('/').next().and_then(|name| {
        name.rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
    });
    let (format, links) = match extension.as_deref() {
        Some("json") => ("json".to_owned(), parse_links_json(&text)?),
        _ => ("csv".to_owned(), parse_links_csv(text.as_bytes())?),
    };
    Ok(TraceLinkDocument {
        source,
        format,
        encoding,
        sha256,
        links,
    })
}

pub fn review_requirement_quality(document: &RequirementDocument) -> RequirementQuality {
    let mut check = CheckResult::new(
        "requirements_quality",
        format!("reviewed {} requirements", document.requirements.len()),
    );
    let mut seen = BTreeMap::<String, usize>::new();
    for requirement in &document.requirements {
        let normalized_id = requirement.id.trim().to_ascii_uppercase();
        if normalized_id.is_empty() {
            check.add(
                Finding::new(
                    "REQ_EMPTY_ID",
                    Severity::Error,
                    "requirement has no stable identifier",
                )
                .at_path(document.source.clone()),
            );
        } else if let Some(previous_line) =
            seen.insert(normalized_id, requirement.source_line.unwrap_or(0))
        {
            check.add(
                Finding::new(
                    "REQ_DUPLICATE_ID",
                    Severity::Error,
                    "requirement identifier occurs more than once",
                )
                .at_path(document.source.clone())
                .with_detail("requirement_id", requirement.id.clone())
                .with_detail("previous_line", previous_line)
                .with_detail("line", requirement.source_line.unwrap_or(0)),
            );
        }
        if requirement.statement.trim().is_empty() {
            check.add(
                Finding::new(
                    "REQ_EMPTY_STATEMENT",
                    Severity::Error,
                    "requirement has no statement",
                )
                .at_path(document.source.clone())
                .with_detail("requirement_id", requirement.id.clone()),
            );
        }
        if requirement.verification_method.is_none() {
            check.add(
                Finding::new(
                    "REQ_MISSING_VERIFICATION_METHOD",
                    Severity::Warning,
                    "requirement has no verification method",
                )
                .at_path(document.source.clone())
                .with_detail("requirement_id", requirement.id.clone()),
            );
        }
        if let Some(status) = &requirement.status
            && !is_known_status(status)
        {
            check.add(
                Finding::new(
                    "REQ_UNKNOWN_STATUS",
                    Severity::Warning,
                    "requirement status is not one of the supported lifecycle values",
                )
                .at_path(document.source.clone())
                .with_detail("requirement_id", requirement.id.clone())
                .with_detail("status", status.clone()),
            );
        }
    }
    if document.requirements.is_empty() {
        check.add(
            Finding::new(
                "REQ_NO_REQUIREMENTS",
                Severity::Error,
                "no requirements were parsed",
            )
            .at_path(document.source.clone()),
        );
    }
    RequirementQuality {
        source: document.source.clone(),
        requirement_count: document.requirements.len(),
        check,
    }
}

pub fn build_traceability_matrix(
    requirements: &RequirementDocument,
    links: Option<&TraceLinkDocument>,
) -> TraceabilityMatrix {
    let mut check = CheckResult::new(
        "requirements_traceability",
        "built requirement traceability matrix",
    );
    let requirement_ids = requirements
        .requirements
        .iter()
        .map(|requirement| requirement.id.trim().to_ascii_uppercase())
        .filter(|id| !id.is_empty())
        .collect::<BTreeSet<_>>();
    let mut covered = BTreeSet::new();
    let mut invalid = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for requirement in &requirements.requirements {
        for target in &requirement.targets {
            if !target.trim().is_empty() {
                covered.insert(requirement.id.trim().to_ascii_uppercase());
                targets.insert(target.trim().to_owned());
            }
        }
    }
    if let Some(document) = links {
        for link in &document.links {
            let requirement_id = link.requirement_id.trim().to_ascii_uppercase();
            if !requirement_ids.contains(&requirement_id) {
                invalid.insert(requirement_id.clone());
                check.add(
                    Finding::new(
                        "TRACE_UNKNOWN_REQUIREMENT",
                        Severity::Error,
                        "trace link references an unknown requirement",
                    )
                    .at_path(document.source.clone())
                    .with_detail("requirement_id", link.requirement_id.clone())
                    .with_detail("target", link.target.clone()),
                );
            } else {
                covered.insert(requirement_id);
            }
            if !link.target.trim().is_empty() {
                targets.insert(link.target.trim().to_owned());
            } else {
                check.add(
                    Finding::new(
                        "TRACE_EMPTY_TARGET",
                        Severity::Error,
                        "trace link has no target",
                    )
                    .at_path(document.source.clone())
                    .with_detail("requirement_id", link.requirement_id.clone()),
                );
            }
            if link.evidence.is_none() {
                check.add(
                    Finding::new(
                        "TRACE_MISSING_EVIDENCE",
                        Severity::Warning,
                        "trace link has no evidence reference",
                    )
                    .at_path(document.source.clone())
                    .with_detail("requirement_id", link.requirement_id.clone())
                    .with_detail("target", link.target.clone()),
                );
            }
        }
    }

    let uncovered = requirement_ids
        .difference(&covered)
        .cloned()
        .collect::<Vec<_>>();
    for id in &uncovered {
        let severity = requirements
            .requirements
            .iter()
            .find(|requirement| requirement.id.trim().eq_ignore_ascii_case(id))
            .and_then(|requirement| requirement.status.as_deref())
            .map(|status| {
                matches!(
                    status.to_ascii_lowercase().as_str(),
                    "approved" | "implemented" | "verified"
                )
            })
            .unwrap_or(false);
        check.add(
            Finding::new(
                "TRACE_REQUIREMENT_UNCOVERED",
                if severity {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                "requirement has no design, test, manufacturing, or other trace target",
            )
            .at_path(requirements.source.clone())
            .with_detail("requirement_id", id.clone()),
        );
    }

    TraceabilityMatrix {
        requirements_source: requirements.source.clone(),
        links_source: links.map(|document| document.source.clone()),
        requirement_count: requirement_ids.len(),
        link_count: links.map(|document| document.links.len()).unwrap_or(0),
        covered_requirement_ids: covered.into_iter().collect(),
        uncovered_requirement_ids: uncovered,
        invalid_requirement_ids: invalid.into_iter().collect(),
        trace_targets: targets.into_iter().collect(),
        check,
    }
}

pub fn analyze_requirement_impact(
    requirements: &RequirementDocument,
    links: Option<&TraceLinkDocument>,
    requirement_id: &str,
) -> DomainResult<ImpactAnalysis> {
    let normalized = requirement_id.trim().to_ascii_uppercase();
    let requirement = requirements
        .requirements
        .iter()
        .find(|requirement| requirement.id.trim().eq_ignore_ascii_case(&normalized))
        .cloned()
        .ok_or_else(|| {
            DomainError::InvalidInput(format!("unknown requirement: {requirement_id}"))
        })?;
    let mut check = CheckResult::new(
        "requirement_impact",
        format!("analyzed impact for {}", requirement.id),
    );
    let mut linked_targets = links
        .into_iter()
        .flat_map(|document| document.links.iter())
        .filter(|link| link.requirement_id.trim().eq_ignore_ascii_case(&normalized))
        .cloned()
        .collect::<Vec<_>>();
    linked_targets.extend(requirement.targets.iter().map(|target| TraceLink {
        requirement_id: requirement.id.clone(),
        target: target.clone(),
        relation: "declared_target".to_owned(),
        evidence: None,
        source_line: requirement.source_line,
    }));
    if linked_targets.is_empty() {
        check.add(
            Finding::new(
                "TRACE_IMPACT_NO_TARGETS",
                Severity::Warning,
                "requirement has no linked impact target",
            )
            .at_path(requirements.source.clone())
            .with_detail("requirement_id", requirement.id.clone()),
        );
    }
    let related = requirements
        .requirements
        .iter()
        .filter(|candidate| candidate.id != requirement.id)
        .filter(|candidate| {
            candidate.tags.iter().any(|tag| {
                requirement
                    .tags
                    .iter()
                    .any(|own| own.eq_ignore_ascii_case(tag))
            })
        })
        .map(|candidate| candidate.id.clone())
        .collect();
    Ok(ImpactAnalysis {
        requirement,
        linked_targets,
        related_requirement_ids: related,
        check,
    })
}

fn parse_requirements_json(text: &str) -> DomainResult<Vec<Requirement>> {
    let value: Value = serde_json::from_str(text).map_err(|error| {
        DomainError::InvalidInput(format!("invalid requirements JSON: {error}"))
    })?;
    let values = value
        .get("requirements")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| value.as_array().cloned())
        .ok_or_else(|| {
            DomainError::InvalidInput(
                "requirements JSON must be an array or contain requirements[]".to_owned(),
            )
        })?;
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| requirement_from_json(value, Some(index + 1)))
        .collect())
}

fn requirement_from_json(value: &Value, line: Option<usize>) -> Requirement {
    Requirement {
        id: string_field(value, &["id", "req_id", "requirement_id"]).unwrap_or_default(),
        title: string_field(value, &["title", "name"]),
        statement: string_field(value, &["statement", "text", "description"]).unwrap_or_default(),
        status: string_field(value, &["status", "lifecycle"]),
        verification_method: string_field(
            value,
            &["verification_method", "verification", "verify_by"],
        ),
        tags: string_list_field(value, &["tags", "labels"]),
        targets: string_list_field(value, &["targets", "linked_targets", "implements"]),
        source_line: line,
    }
}

fn parse_requirements_csv(bytes: &[u8]) -> DomainResult<Vec<Requirement>> {
    let (text, _) = decode_text(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(DomainError::Csv)?
        .iter()
        .map(normalize_header)
        .collect::<Vec<_>>();
    let id = find_column(&headers, &["id", "reqid", "requirementid"])
        .ok_or_else(|| DomainError::InvalidInput("requirements CSV has no id column".to_owned()))?;
    let statement = find_column(
        &headers,
        &["statement", "text", "description", "requirement"],
    );
    let title = find_column(&headers, &["title", "name"]);
    let status = find_column(&headers, &["status", "lifecycle"]);
    let verification = find_column(
        &headers,
        &["verificationmethod", "verification", "verifyby"],
    );
    let tags = find_column(&headers, &["tags", "labels"]);
    let targets = find_column(&headers, &["targets", "linkedtargets", "implements"]);
    let mut requirements = Vec::new();
    for (index, row) in reader.records().enumerate() {
        let row = row.map_err(DomainError::Csv)?;
        requirements.push(Requirement {
            id: row.get(id).unwrap_or_default().trim().to_owned(),
            title: title.and_then(|column| non_empty(row.get(column).unwrap_or_default())),
            statement: statement
                .and_then(|column| non_empty(row.get(column).unwrap_or_default()))
                .unwrap_or_default(),
            status: status.and_then(|column| non_empty(row.get(column).unwrap_or_default())),
            verification_method: verification
                .and_then(|column| non_empty(row.get(column).unwrap_or_default())),
            tags: tags
                .map(|column| split_list(row.get(column).unwrap_or_default()))
                .unwrap_or_default(),
            targets: targets
                .map(|column| split_list(row.get(column).unwrap_or_default()))
                .unwrap_or_default(),
            source_line: Some(index + 2),
        });
    }
    Ok(requirements)
}

fn parse_requirements_markdown(text: &str) -> Vec<Requirement> {
    let mut requirements = Vec::new();
    let mut current: Option<Requirement> = None;
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if let Some((id, title, statement)) = parse_requirement_line(line) {
            if let Some(requirement) = current.take() {
                requirements.push(requirement);
            }
            current = Some(Requirement {
                id,
                title,
                statement,
                status: None,
                verification_method: None,
                tags: Vec::new(),
                targets: Vec::new(),
                source_line: Some(line_index + 1),
            });
        } else if let Some(requirement) = &mut current {
            if let Some(value) = line
                .strip_prefix("Status:")
                .or_else(|| line.strip_prefix("状态:"))
            {
                requirement.status = non_empty(value);
            } else if let Some(value) = line
                .strip_prefix("Verification:")
                .or_else(|| line.strip_prefix("验证:"))
            {
                requirement.verification_method = non_empty(value);
            } else if !line.is_empty() && !line.starts_with("<!--") {
                if !requirement.statement.is_empty() {
                    requirement.statement.push(' ');
                }
                requirement.statement.push_str(line);
            }
        }
    }
    if let Some(requirement) = current {
        requirements.push(requirement);
    }
    requirements
}

fn parse_requirement_line(line: &str) -> Option<(String, Option<String>, String)> {
    let token_start = line
        .find("REQ-")
        .or_else(|| line.find("REQ_"))
        .or_else(|| line.find("SYS-"))
        .or_else(|| line.find("HW-"))
        .or_else(|| line.find("SW-"))?;
    let token = &line[token_start..];
    let id_end = token
        .char_indices()
        .find(|(_, character)| {
            !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.')
        })
        .map(|(index, _)| index)
        .unwrap_or(token.len());
    let id = token[..id_end].to_owned();
    if id.len() < 5 {
        return None;
    }
    let remainder = token[id_end..]
        .trim_start_matches([':', '-', ' ', '\t'])
        .trim();
    let (title, statement) = if line.starts_with('#') {
        if let Some((title, statement)) = remainder.split_once(" - ") {
            (non_empty(title), statement.trim().to_owned())
        } else {
            (non_empty(remainder), String::new())
        }
    } else {
        (None, remainder.to_owned())
    };
    Some((id, title, statement))
}

fn parse_links_json(text: &str) -> DomainResult<Vec<TraceLink>> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| DomainError::InvalidInput(format!("invalid trace links JSON: {error}")))?;
    let values = value
        .get("links")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| value.as_array().cloned())
        .ok_or_else(|| {
            DomainError::InvalidInput(
                "trace links JSON must be an array or contain links[]".to_owned(),
            )
        })?;
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| TraceLink {
            requirement_id: string_field(value, &["requirement_id", "requirement", "req_id", "id"])
                .unwrap_or_default(),
            target: string_field(value, &["target", "object", "artifact"]).unwrap_or_default(),
            relation: string_field(value, &["relation", "type", "kind"])
                .unwrap_or_else(|| "traces_to".to_owned()),
            evidence: string_field(value, &["evidence", "evidence_path", "source"]),
            source_line: Some(index + 1),
        })
        .collect())
}

fn parse_links_csv(bytes: &[u8]) -> DomainResult<Vec<TraceLink>> {
    let (text, _) = decode_text(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(DomainError::Csv)?
        .iter()
        .map(normalize_header)
        .collect::<Vec<_>>();
    let requirement = find_column(&headers, &["requirementid", "requirement", "reqid"])
        .ok_or_else(|| {
            DomainError::InvalidInput("trace links CSV has no requirement_id column".to_owned())
        })?;
    let target = find_column(&headers, &["target", "object", "artifact"]).ok_or_else(|| {
        DomainError::InvalidInput("trace links CSV has no target column".to_owned())
    })?;
    let relation = find_column(&headers, &["relation", "type", "kind"]);
    let evidence = find_column(&headers, &["evidence", "evidencepath", "source"]);
    let mut links = Vec::new();
    for (index, row) in reader.records().enumerate() {
        let row = row.map_err(DomainError::Csv)?;
        links.push(TraceLink {
            requirement_id: row.get(requirement).unwrap_or_default().trim().to_owned(),
            target: row.get(target).unwrap_or_default().trim().to_owned(),
            relation: relation
                .and_then(|column| non_empty(row.get(column).unwrap_or_default()))
                .unwrap_or_else(|| "traces_to".to_owned()),
            evidence: evidence.and_then(|column| non_empty(row.get(column).unwrap_or_default())),
            source_line: Some(index + 2),
        });
    }
    Ok(links)
}

fn decode_text(bytes: &[u8]) -> (String, String) {
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
    (
        decoded.into_owned(),
        if malformed {
            "gb18030-with-replacement"
        } else {
            "gb18030"
        }
        .to_owned(),
    )
}

fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str).and_then(non_empty))
}

fn string_list_field(value: &Value, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .map(|value| match value {
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(non_empty)
                .collect(),
            Value::String(text) => split_list(text),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(|character: char| {
            character == ',' || character == ';' || character == '|' || character.is_whitespace()
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn normalize_header(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn find_column(headers: &[String], aliases: &[&str]) -> Option<usize> {
    headers
        .iter()
        .position(|header| aliases.iter().any(|alias| header == alias))
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn is_known_status(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "draft" | "proposed" | "approved" | "implemented" | "verified" | "obsolete" | "rejected"
    )
}
