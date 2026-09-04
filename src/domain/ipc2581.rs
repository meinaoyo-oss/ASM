use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{CheckResult, DomainResult, Finding, Severity};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Ipc2581Validation {
    pub source: String,
    pub root_element: Option<String>,
    pub revision: Option<String>,
    pub layer_count: usize,
    pub component_count: usize,
    pub bom_item_count: usize,
    pub check: CheckResult,
}

/// Reads the IPC-2581 envelope without materializing the full XML document.
/// It deliberately reports only the release-relevant counts needed by the MVP.
pub fn validate_ipc2581(
    bytes: &[u8],
    source: impl Into<String>,
) -> DomainResult<Ipc2581Validation> {
    let source = source.into();
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut root_element = None;
    let mut revision = None;
    let mut layer_count = 0;
    let mut component_count = 0;
    let mut bom_item_count = 0;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) | Event::Empty(element) => {
                let name = element_name(&element);
                if root_element.is_none() {
                    revision = attribute_value(&element, "revision")
                        .or_else(|| attribute_value(&element, "version"));
                    root_element = Some(name.clone());
                }
                match name.as_str() {
                    "layer" => layer_count += 1,
                    "component" => component_count += 1,
                    "bomitem" | "bom_item" => bom_item_count += 1,
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    let mut check = CheckResult::new("ipc2581", "parsed IPC-2581 XML release data");
    let is_ipc2581 = root_element
        .as_deref()
        .map(|name| name.contains("ipc") && name.contains("2581"))
        .unwrap_or(false);
    if !is_ipc2581 {
        check.add(
            Finding::new(
                "IPC2581_INVALID_ROOT",
                Severity::Error,
                "XML root is not an IPC-2581 document",
            )
            .at_path(source.clone()),
        );
    }
    if layer_count == 0 {
        check.add(
            Finding::new(
                "IPC2581_NO_LAYERS",
                Severity::Warning,
                "IPC-2581 document contains no Layer elements",
            )
            .at_path(source.clone()),
        );
    }
    if component_count == 0 {
        check.add(
            Finding::new(
                "IPC2581_NO_COMPONENTS",
                Severity::Warning,
                "IPC-2581 document contains no Component elements",
            )
            .at_path(source.clone()),
        );
    }
    Ok(Ipc2581Validation {
        source,
        root_element,
        revision,
        layer_count,
        component_count,
        bom_item_count,
        check,
    })
}

fn element_name(element: &BytesStart<'_>) -> String {
    let qualified_name = element.name().as_ref().to_vec();
    let raw = String::from_utf8_lossy(&qualified_name);
    raw.rsplit(':').next().unwrap_or(&raw).to_ascii_lowercase()
}

fn attribute_value(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        let name = String::from_utf8_lossy(attribute.key.as_ref()).to_ascii_lowercase();
        (name == key).then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}
