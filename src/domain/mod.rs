//! Offline PCB release validation domain logic.
//!
//! This module intentionally does not depend on MCP transport types.  The
//! binary adapter can use these serializable values directly in tool results.

mod bom;
mod gerber;
mod ipc2581;
mod package;
mod profile;
mod release;
mod types;

pub use bom::{
    BomCplComparison, BomDocument, BomEntry, BomValidation, CplDocument, Placement,
    compare_bom_cpl, parse_bom, parse_cpl, validate_bom,
};
pub use gerber::{
    ExcellonValidation, GerberValidation, PcbFileRole, classify_pcb_file, validate_excellon,
    validate_gerber_set,
};
pub use ipc2581::{Ipc2581Validation, validate_ipc2581};
pub use package::{
    PackageInventory, PackageKind, PackageLimits, inspect_package, read_package_member,
};
pub use profile::{ManufacturingProfile, ProfileRules};
pub use release::{ReleaseKind, ReleaseReport, ReleaseRequest, validate_release};
pub use types::{Artifact, CheckResult, DomainError, Finding, Severity, Status};
