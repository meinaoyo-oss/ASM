use std::collections::BTreeMap;
use std::fmt;
use std::io;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug)]
pub enum DomainError {
    Io(io::Error),
    Csv(csv::Error),
    Zip(zip::result::ZipError),
    Xml(quick_xml::Error),
    InvalidInput(String),
    LimitExceeded(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Csv(error) => write!(f, "CSV error: {error}"),
            Self::Zip(error) => write!(f, "ZIP error: {error}"),
            Self::Xml(error) => write!(f, "XML error: {error}"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::LimitExceeded(message) => write!(f, "resource limit exceeded: {message}"),
        }
    }
}

impl std::error::Error for DomainError {}

impl From<io::Error> for DomainError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<csv::Error> for DomainError {
    fn from(value: csv::Error) -> Self {
        Self::Csv(value)
    }
}

impl From<zip::result::ZipError> for DomainError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<quick_xml::Error> for DomainError {
    fn from(value: quick_xml::Error) -> Self {
        Self::Xml(value)
    }
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

impl Severity {
    pub fn status(self) -> Status {
        match self {
            Self::Info => Status::Pass,
            Self::Warning => Status::Warn,
            Self::Error | Self::Critical => Status::Fail,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Pass,
    Warn,
    Fail,
    Skipped,
}

impl Status {
    pub fn combine(self, other: Self) -> Self {
        use Status::{Fail, Pass, Skipped, Warn};
        match (self, other) {
            (Fail, _) | (_, Fail) => Fail,
            (Warn, _) | (_, Warn) => Warn,
            (Pass, _) | (_, Pass) => Pass,
            (Skipped, Skipped) => Skipped,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
}

impl Finding {
    pub fn new(code: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            path: None,
            details: BTreeMap::new(),
        }
    }

    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct CheckResult {
    pub id: String,
    pub status: Status,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
}

impl CheckResult {
    pub fn new(id: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Pass,
            summary: summary.into(),
            findings: Vec::new(),
        }
    }

    pub fn skipped(id: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: Status::Skipped,
            summary: summary.into(),
            findings: Vec::new(),
        }
    }

    pub fn add(&mut self, finding: Finding) {
        self.status = self.status.combine(finding.severity.status());
        self.findings.push(finding);
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct Artifact {
    pub path: String,
    pub role: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser_status: Option<Status>,
}
