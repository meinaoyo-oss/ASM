use std::{fs, io::Write, path::PathBuf};

use electronics_manufacturing_mcp::domain::{
    ManufacturingProfile, PackageLimits, ReleaseKind, ReleaseRequest, Severity, Status,
    analyze_requirement_impact, build_traceability_matrix, classify_pcb_file, compare_bom_cpl,
    compare_kicad_revisions, inspect_kicad_project, inspect_package, parse_bom, parse_cpl,
    parse_kicad_document, parse_requirements, parse_trace_links, review_bom_risk,
    review_kicad_power_tree, review_requirement_quality, trace_kicad_signal, validate_gerber_set,
    validate_ipc2581, validate_release, validate_spice_netlist,
};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

#[test]
fn valid_jlcpcb_assembly_passes_release_gate() {
    let report = validate_release(
        fixture("valid_assembly"),
        ReleaseRequest {
            profile: ManufacturingProfile::Jlcpcb,
            release_kind: ReleaseKind::Assembly,
            expected_copper_layers: Some(2),
            ..ReleaseRequest::default()
        },
    )
    .expect("valid fixture should be readable");

    assert_eq!(report.status, Status::Pass, "{:#?}", report.findings);
    assert_eq!(report.artifacts.len(), 7);
    assert!(
        report
            .artifacts
            .iter()
            .all(|artifact| artifact.role != "unknown")
    );
}

#[test]
fn bom_cpl_mismatch_reports_stable_errors() {
    let bom_path = fixture("bom_cpl_mismatch/bom.csv");
    let cpl_path = fixture("bom_cpl_mismatch/cpl.csv");
    let bom = parse_bom(
        &fs::read(&bom_path).unwrap(),
        bom_path.display().to_string(),
    )
    .unwrap();
    let cpl = parse_cpl(
        &fs::read(&cpl_path).unwrap(),
        cpl_path.display().to_string(),
    )
    .unwrap();
    let result = compare_bom_cpl(&bom, &cpl, ManufacturingProfile::Jlcpcb);
    let codes = result
        .check
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(result.check.status, Status::Fail);
    assert!(codes.contains(&"CPL_INVALID_ROTATION"));
    assert!(codes.contains(&"CPL_INVALID_LAYER"));
    assert!(codes.contains(&"CPL_MISSING_REFERENCE"));
    assert!(codes.contains(&"CPL_UNKNOWN_REFERENCE"));
}

#[test]
fn malformed_gerber_is_not_accepted() {
    let path = fixture("malformed_gerber/broken.gtl");
    let result = validate_gerber_set(
        &[(path.display().to_string(), fs::read(&path).unwrap())],
        ManufacturingProfile::Generic,
        Some(1),
    );

    assert_eq!(result.check.status, Status::Fail);
    assert!(
        result
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "GERBER_PARSE_FAILED")
    );
}

#[test]
fn duplicate_top_copper_cannot_satisfy_layer_count() {
    let gerber = b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.100*%\nM02*\n".to_vec();
    let result = validate_gerber_set(
        &[
            ("first.gtl".to_owned(), gerber.clone()),
            ("second.gtl".to_owned(), gerber),
        ],
        ManufacturingProfile::Generic,
        Some(2),
    );
    let codes = result
        .check
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(result.copper_layer_count, 1);
    assert!(codes.contains(&"GERBER_DUPLICATE_TOP_COPPER"));
    assert!(codes.contains(&"GERBER_INSUFFICIENT_COPPER_LAYERS"));
}

#[test]
fn non_ipc_xml_fails_ipc2581_validation() {
    let path = fixture("invalid_ipc2581/not-ipc.xml");
    let result = validate_ipc2581(&fs::read(&path).unwrap(), path.display().to_string()).unwrap();

    assert_eq!(result.check.status, Status::Fail);
    assert!(result.check.findings.iter().any(|finding| {
        finding.code == "IPC2581_INVALID_ROOT" && finding.severity == Severity::Error
    }));
}

#[test]
fn csv_accepts_utf8_bom_and_gb18030() {
    let utf8_bom = b"\xef\xbb\xbfDesignator,Value,Footprint,MPN\nR1,10k,0603,C25804\n";
    let utf8 = parse_bom(utf8_bom, "utf8-bom.csv").unwrap();
    assert_eq!(utf8.encoding, "utf-8-bom");

    let (encoded, _, had_errors) = encoding_rs::GB18030
        .encode("Designator,Value,Footprint,MPN\nR1,\u{7535}\u{963b},0603,C25804\n");
    assert!(!had_errors);
    let gb18030 = parse_bom(encoded.as_ref(), "gb18030.csv").unwrap();
    assert_eq!(gb18030.encoding, "gb18030");
    assert_eq!(
        gb18030.entries[0].value.as_deref(),
        Some("\u{7535}\u{963b}")
    );
}

#[test]
fn dnp_and_populate_headers_use_opposite_boolean_semantics() {
    let dnp = parse_bom(
        b"Designator,Value,MPN,DNP\nR1,10k,C25804,No\nR2,10k,C25804,Yes\n",
        "dnp.csv",
    )
    .unwrap();
    assert!(!dnp.entries[0].do_not_place);
    assert!(dnp.entries[1].do_not_place);

    let populate = parse_bom(
        b"Designator,Value,MPN,Populate\nR1,10k,C25804,No\nR2,10k,C25804,Yes\n",
        "populate.csv",
    )
    .unwrap();
    assert!(populate.entries[0].do_not_place);
    assert!(!populate.entries[1].do_not_place);
}

#[test]
fn bom_quantity_must_match_designator_count() {
    let bom = parse_bom(
        b"Designator,Quantity,Value,Footprint,MPN\nR1 R2,1,10k,0603,C25804\n",
        "quantity.csv",
    )
    .unwrap();
    let validation =
        electronics_manufacturing_mcp::domain::validate_bom(&bom, ManufacturingProfile::Jlcpcb);

    assert_eq!(validation.check.status, Status::Fail);
    assert!(
        validation
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "BOM_QUANTITY_MISMATCH")
    );
}

#[test]
fn zip_path_traversal_is_rejected() {
    let temporary = tempfile::tempdir().unwrap();
    let archive_path = temporary.path().join("unsafe.zip");
    let file = fs::File::create(&archive_path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file("../escape.gtl", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"M02*\n").unwrap();
    archive.finish().unwrap();

    let result = inspect_package(&archive_path, PackageLimits::default());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("unsafe package member path")
    );
}

#[test]
fn requirements_quality_and_traceability_report_gaps() {
    let requirements = parse_requirements(
        br#"{
          "requirements": [
            {"id":"REQ-001","title":"Power","statement":"Input shall tolerate 24 V","status":"approved","verification_method":"test","tags":["power"]},
            {"id":"REQ-001","statement":"duplicate"},
            {"id":"REQ-002","statement":"Output shall be isolated","tags":["safety"]}
          ]
        }"#,
        "requirements.json",
    )
    .unwrap();
    let quality = review_requirement_quality(&requirements);
    assert_eq!(quality.check.status, Status::Fail);
    assert!(
        quality
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "REQ_DUPLICATE_ID")
    );
    assert!(
        quality
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "REQ_MISSING_VERIFICATION_METHOD")
    );

    let links = parse_trace_links(
        br#"[{"requirement_id":"REQ-001","target":"test:T-001","relation":"verified_by","evidence":"results/T-001.json"}]"#,
        "links.json",
    )
    .unwrap();
    let matrix = build_traceability_matrix(&requirements, Some(&links));
    assert!(
        matrix
            .covered_requirement_ids
            .contains(&"REQ-001".to_owned())
    );
    assert!(
        matrix
            .uncovered_requirement_ids
            .contains(&"REQ-002".to_owned())
    );
    assert!(
        matrix
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "TRACE_REQUIREMENT_UNCOVERED")
    );
    let impact = analyze_requirement_impact(&requirements, Some(&links), "REQ-001").unwrap();
    assert_eq!(impact.linked_targets[0].target, "test:T-001");
}

#[test]
fn markdown_requirements_are_ingested() {
    let document = parse_requirements(
        b"# Requirements\n\n## REQ-100: Thermal\nBoard shall operate at 85 C.\nVerification: analysis\nStatus: approved\n",
        "requirements.md",
    )
    .unwrap();
    assert_eq!(document.requirements.len(), 1);
    assert_eq!(document.requirements[0].id, "REQ-100");
    assert_eq!(document.requirements[0].title.as_deref(), Some("Thermal"));
    assert!(document.requirements[0].statement.contains("85 C"));
}

#[test]
fn bom_risk_review_distinguishes_eol_and_missing_evidence() {
    let document = parse_bom(
        b"Designator,Quantity,Value,Footprint,MPN,Manufacturer,Lifecycle,Supplier,Alternate\nU1,1,MCU,QFN,STM32,Eve,EOL,,\nR1,1,10k,0603,R-10k,Acme,active,DigiKey,R-10k-alt\n",
        "risk.csv",
    )
    .unwrap();
    let report = review_bom_risk(&document, ManufacturingProfile::Jlcpcb);
    assert_eq!(report.check.status, Status::Fail);
    assert!(
        report
            .risks
            .iter()
            .any(|risk| risk.category == "lifecycle" && risk.severity == Severity::Error)
    );
    assert!(
        report
            .risks
            .iter()
            .any(|risk| risk.category == "supply" && risk.references == vec!["U1"])
    );
}

#[test]
fn spice_netlist_checks_ground_analysis_and_end() {
    let valid = validate_spice_netlist(
        b"RC filter\nV1 in 0 AC 1\nR1 in out 1k\nC1 out 0 1u\n.ac dec 10 10 1Meg\n.end\n",
        "filter.cir",
    );
    assert_eq!(valid.check.status, Status::Pass);
    assert_eq!(valid.component_count, 3);
    assert!(valid.analyses.contains(&"ac".to_owned()));

    let invalid = validate_spice_netlist(b"V1 in out 5\nR1 in out 1k\n", "bad.cir");
    assert_eq!(invalid.check.status, Status::Fail);
    assert!(
        invalid
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "SPICE_MISSING_GROUND")
    );
    assert!(
        invalid
            .check
            .findings
            .iter()
            .any(|finding| finding.code == "SPICE_MISSING_END")
    );
}

#[test]
fn kicad_native_parser_extracts_components_layers_and_power() {
    let pcb_path = fixture("kicad/rev-a/board.kicad_pcb");
    let sch_path = fixture("kicad/rev-a/board.kicad_sch");
    let pcb = parse_kicad_document(
        &fs::read(&pcb_path).unwrap(),
        pcb_path.display().to_string(),
    )
    .unwrap();
    let sch = parse_kicad_document(
        &fs::read(&sch_path).unwrap(),
        sch_path.display().to_string(),
    )
    .unwrap();
    assert_eq!(pcb.components.len(), 2);
    assert_eq!(
        pcb.components[0].footprint.as_deref(),
        Some("Resistor_SMD:R_0603_1608Metric")
    );
    assert!(pcb.layers.contains(&"F.Cu".to_owned()));
    assert!(sch.labels.contains(&"VCC_3V3".to_owned()));
    let project = inspect_kicad_project(vec![pcb, sch], "rev-a");
    assert_eq!(project.component_count, 4);
    let power = review_kicad_power_tree(&project);
    assert!(power.power_nets.contains(&"VCC_3V3".to_owned()));
    assert!(power.ground_nets.contains(&"GND".to_owned()));
    let trace = trace_kicad_signal(&project, "R1");
    assert_eq!(trace.matched_components.len(), 2);
}

#[test]
fn kicad_revision_diff_reports_changes() {
    let left_path = fixture("kicad/rev-a/board.kicad_pcb");
    let right_path = fixture("kicad/rev-b/board.kicad_pcb");
    let left = parse_kicad_document(
        &fs::read(&left_path).unwrap(),
        left_path.display().to_string(),
    )
    .unwrap();
    let right = parse_kicad_document(
        &fs::read(&right_path).unwrap(),
        right_path.display().to_string(),
    )
    .unwrap();
    let left = inspect_kicad_project(vec![left], "rev-a");
    let right = inspect_kicad_project(vec![right], "rev-b");
    let diff = compare_kicad_revisions(&left, &right);
    assert_eq!(diff.added_components, vec!["J1"]);
    assert_eq!(diff.changed_components, vec!["R1"]);
    assert!(diff.added_nets.contains(&"VCC_5V".to_owned()));
    assert_eq!(diff.check.status, Status::Warn);
}

#[test]
fn kicad_parser_preserves_utf8_properties_and_package_roles() {
    let document = parse_kicad_document(
        "(kicad_sch (version 20231120) (symbol (lib_id \"Device:R\") (property \"Reference\" \"R1\") (property \"Value\" \"电阻\")))".as_bytes(),
        "utf8.kicad_sch",
    )
    .unwrap();
    assert_eq!(document.components[0].value.as_deref(), Some("电阻"));
    assert_eq!(classify_pcb_file("board.kicad_pcb").as_str(), "kicad_pcb");
    assert_eq!(
        classify_pcb_file("requirements.json").as_str(),
        "requirements"
    );
    assert_eq!(classify_pcb_file("filter.cir").as_str(), "spice_netlist");
}
