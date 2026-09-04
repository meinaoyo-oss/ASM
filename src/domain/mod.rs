//! Offline PCB release validation domain logic.
//!
//! This module intentionally does not depend on MCP transport types.  The
//! binary adapter can use these serializable values directly in tool results.

mod bom;
mod bom_risk;
mod gerber;
mod ipc2581;
mod kicad_fabrication;
mod kicad_native;
mod kicad_review;
mod package;
mod profile;
mod release;
mod spice;
mod traceability;
mod types;

pub use bom::{
    BomCplComparison, BomDocument, BomEntry, BomValidation, CplDocument, Placement,
    compare_bom_cpl, parse_bom, parse_cpl, validate_bom,
};
pub use bom_risk::{BomRisk, BomRiskReport, review_bom_risk};
pub use gerber::{
    ExcellonValidation, GerberValidation, PcbFileRole, classify_pcb_file, validate_excellon,
    validate_gerber_set,
};
pub use ipc2581::{Ipc2581Validation, validate_ipc2581};
pub use kicad_fabrication::{DfmThresholds, PcbDfmDfaDftReview, review_pcb_dfm_dfa_dft};
pub use kicad_native::{
    KicadComponent, KicadConnectivity, KicadConnectivityNet, KicadConsistencyReport, KicadDocument,
    KicadDocumentKind, KicadLabel, KicadNet, KicadPad, KicadPin, KicadPinPadMismatch, KicadPoint,
    KicadPowerTree, KicadProjectSnapshot, KicadRevisionDiff, KicadSignalTrace, KicadTrack,
    KicadVia, KicadWire, analyze_kicad_connectivity, compare_kicad_revisions,
    compare_kicad_schematic_pcb, inspect_kicad_project, parse_kicad_document,
    review_kicad_power_tree, trace_kicad_signal,
};
pub use kicad_review::{KicadDesignReview, review_kicad_design};
pub use package::{
    PackageInventory, PackageKind, PackageLimits, inspect_package, read_package_member,
};
pub use profile::{ManufacturingProfile, ProfileRules};
pub use release::{ReleaseKind, ReleaseReport, ReleaseRequest, validate_release};
pub use spice::{SpiceComponent, SpiceValidation, validate_spice_netlist};
pub use traceability::{
    ImpactAnalysis, Requirement, RequirementDocument, RequirementQuality, TraceLink,
    TraceLinkDocument, TraceabilityMatrix, analyze_requirement_impact, build_traceability_matrix,
    parse_requirements, parse_trace_links, review_requirement_quality,
};
pub use types::{Artifact, CheckResult, DomainError, Finding, Severity, Status};
