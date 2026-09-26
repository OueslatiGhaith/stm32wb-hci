use super::*;
use crate::annotations::{Annotations, LayoutSlot};

const SAMPLE: &str = r#"
platform = "stm32wb"

[[releases]]
version = "1.15.0"
tag = "v1.15.0"
commit = "0000000000000000000000000000000000000000"

[[releases]]
version = "1.16.0"
tag = "v1.16.0"
commit = "0000000000000000000000000000000000000000"

[[releases]]
version = "1.17.0"
tag = "v1.17.0"
commit = "0000000000000000000000000000000000000000"

[[releases]]
version = "1.18.0"
tag = "v1.18.0"
commit = "0000000000000000000000000000000000000000"

[[binaries]]
family = "wb5x"
profile = "full-extended"
file = "stm32wb5x_BLE_Stack_full_extended_fw.bin"
releases = "1.15.0..=1.18.0"

[[commands]]
scope = "vendor"
opcode = "0xFC83"

[[commands.names]]
releases = "1.15.0..=1.18.0"
name = "aci_gap_set_discoverable"

[[commands.availability]]
releases = "1.15.0..=1.16.0"
profiles = ["full-extended", "full"]

[[commands.availability]]
releases = "1.17.0..=1.18.0"
profiles = ["full-extended"]

[[commands.definitions]]
releases = "1.15.0..=1.17.0"
completion = "command-complete"
params = [
    "Advertising_Type: u8",
    "Local_Name_Length: u8",
    "Local_Name: [u8; Local_Name_Length] (capacity 242)",
]
returns = ["Status: u8"]

[[commands.definitions]]
releases = "1.18.0"
completion = "command-complete"
params = ["Advertising_Type: u8"]
returns = ["Status: u8"]

[[events]]
scope = "system"
code = "0x9200"

[[events.names]]
releases = "1.15.0..=1.18.0"
name = "SHCI_SUB_EVT_CODE_READY"

[[events.availability]]
releases = "1.15.0..=1.18.0"
profiles = ["full-extended", "full", "light", "hci-layer-extended", "hci-layer", "hci-adv-scan"]

[[events.definitions]]
releases = "1.15.0..=1.18.0"
payload = { unresolved = "field sysevt_ready_rsp is an enum" }
"#;

fn sample() -> Catalog {
    Catalog::from_toml(SAMPLE).unwrap()
}

fn fields(fields: &[&str]) -> Layout {
    Layout::Fields(fields.iter().map(|field| field.parse().unwrap()).collect())
}

#[test]
fn lookups_follow_histories() {
    let catalog = sample();
    let command = catalog.command_named("aci_gap_set_discoverable").unwrap();
    assert_eq!(catalog.command(CommandScope::Vendor, 0xFC83), Some(command));
    assert_eq!(command.name(), "aci_gap_set_discoverable");
    assert_eq!(
        command
            .definition_at(Version::new(1, 17, 0))
            .unwrap()
            .params
            .fields()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        profiles_at(&command.availability, Version::new(1, 17, 0)),
        Some([Profile::FullExtended].as_slice())
    );
    assert_eq!(command.definition_at(Version::new(1, 19, 0)), None);

    let event = catalog.event(EventScope::System, 0x9200).unwrap();
    assert_eq!(catalog.event_named("SHCI_SUB_EVT_CODE_READY"), Some(event));
    assert!(matches!(
        event.definition_at(Version::new(1, 15, 0)).unwrap().payload,
        Layout::Unresolved(_)
    ));
}

#[test]
fn toml_round_trips() {
    let catalog = sample();
    let source = catalog.to_toml().unwrap();
    assert!(source.contains("opcode = \"0xFC83\""), "{source}");
    assert!(source.contains("\"Local_Name: [u8; Local_Name_Length] (capacity 242)\""));
    assert!(source.contains("unresolved = \"field sysevt_ready_rsp is an enum\""));
    assert_eq!(Catalog::from_toml(&source).unwrap(), catalog);
}

#[test]
fn validation_rejects_gaps_between_histories() {
    let mut catalog = sample();
    catalog.commands[0].availability.pop();
    let error = catalog.validate().unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Invalid);
    assert!(error.message().contains("availability"), "{error}");

    let mut catalog = sample();
    catalog.commands[0].names[0].releases = "1.15.0..=1.17.0".parse().unwrap();
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    catalog.commands[0].definitions[0].completion = Completion::CommandStatus;
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    catalog.commands[0].definitions[1].params = fields(&["Data: [u8; Missing] (capacity 2)"]);
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    catalog.commands[0].opcode = 0x0C03;
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    catalog.releases.swap(0, 1);
    assert!(catalog.validate().is_err());
}

#[test]
fn validation_rejects_inconsistent_binaries() {
    let mut catalog = sample();
    catalog.binaries[0].profile = Profile::Light;
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    let mut overlapping = catalog.binaries[0].clone();
    overlapping.releases = "1.18.0".parse().unwrap();
    catalog.binaries.push(overlapping);
    assert!(catalog.validate().is_err());

    let mut catalog = sample();
    catalog.binaries[0].releases = "1.15.0..=1.19.0".parse().unwrap();
    assert!(catalog.validate().is_err());
}

#[test]
fn parsing_rejects_unknown_keys_and_malformed_codes() {
    let unknown = SAMPLE.replace(
        "platform = \"stm32wb\"",
        "platform = \"stm32wb\"\nextra = 1",
    );
    assert_eq!(
        Catalog::from_toml(&unknown).unwrap_err().kind(),
        ErrorKind::Parse
    );
    let decimal = SAMPLE.replace("\"0xFC83\"", "\"64643\"");
    assert_eq!(
        Catalog::from_toml(&decimal).unwrap_err().kind(),
        ErrorKind::Parse
    );
}

fn source(version: &str) -> ReleaseSource {
    let version: Version = version.parse().unwrap();
    ReleaseSource {
        version,
        tag: version.cube_tag(),
        commit: "0".repeat(40),
    }
}

fn set_discoverable(profiles: &[Profile], params: Layout) -> SnapshotCommand {
    SnapshotCommand {
        scope: CommandScope::Vendor,
        opcode: 0xFC83,
        name: "aci_gap_set_discoverable".into(),
        profiles: profiles.to_vec(),
        completion: Completion::CommandComplete,
        params,
        returns: Some(fields(&["Status: u8"])),
        structs: Structs::new(),
    }
}

fn snapshot(version: &str, commands: Vec<SnapshotCommand>) -> Snapshot {
    Snapshot {
        source: source(version),
        binaries: vec![SnapshotBinary {
            family: Family::Wb5x,
            profile: Profile::FullExtended,
        }],
        commands,
        events: vec![SnapshotEvent {
            scope: EventScope::System,
            code: 0x9200,
            name: "SHCI_SUB_EVT_CODE_READY".into(),
            // Unsorted and duplicated on purpose: merging normalizes profiles.
            profiles: [Profile::ALL.as_slice(), &[Profile::Full]].concat(),
            payload: Layout::Unresolved("field sysevt_ready_rsp is an enum".into()),
            structs: Structs::new(),
        }],
    }
}

#[test]
fn merge_produces_independent_histories() {
    let v1 = fields(&[
        "Advertising_Type: u8",
        "Local_Name_Length: u8",
        "Local_Name: [u8; Local_Name_Length] (capacity 242)",
    ]);
    let v2 = fields(&["Advertising_Type: u8"]);
    let full = [Profile::Full, Profile::FullExtended];
    let merged = merge_snapshots(
        Platform::Stm32wb,
        vec![
            snapshot("1.16.0", vec![set_discoverable(&full, v1.clone())]),
            snapshot("1.15.0", vec![set_discoverable(&full, v1.clone())]),
            snapshot(
                "1.17.0",
                vec![set_discoverable(&[Profile::FullExtended], v1)],
            ),
            snapshot(
                "1.18.0",
                vec![set_discoverable(&[Profile::FullExtended], v2)],
            ),
        ],
    )
    .unwrap();
    // Availability and definitions change in different releases, so each
    // history splits on its own boundary.
    assert_eq!(merged, sample());
}

#[test]
fn merge_rejects_duplicates_within_a_release() {
    let command = set_discoverable(&[Profile::FullExtended], fields(&[]));
    let error = merge_snapshots(
        Platform::Stm32wb,
        vec![snapshot("1.15.0", vec![command.clone(), command])],
    )
    .unwrap_err();
    assert!(error.message().contains("duplicate"), "{error}");
}

fn audit(source: &str) -> Result<(), Error> {
    Annotations::from_toml(source)?.audit(&sample())
}

#[test]
fn annotations_fill_unresolved_layouts() {
    let source = r#"
        [[annotations]]
        event = "SHCI_SUB_EVT_CODE_READY"
        source = "AN5289 section 4.8.1"
        reason = "the enum is transmitted as one byte"
        payload = ["sysevt_ready_rsp: u8"]

        [[annotations]]
        command = "aci_gap_set_discoverable"
        releases = "1.18.0"
        source = "x"
        reason = "y"
        exclude = true
    "#;
    audit(source).unwrap();

    let annotations = Annotations::from_toml(source).unwrap();
    let (annotation, fields) = annotations
        .layout(
            "SHCI_SUB_EVT_CODE_READY",
            false,
            LayoutSlot::Payload,
            Version::new(1, 15, 0),
        )
        .unwrap();
    assert_eq!(annotation.source, "AN5289 section 4.8.1");
    assert_eq!(fields.len(), 1);
    assert!(
        annotations
            .layout(
                "SHCI_SUB_EVT_CODE_READY",
                true,
                LayoutSlot::Payload,
                Version::new(1, 15, 0),
            )
            .is_none()
    );

    let name = "aci_gap_set_discoverable";
    assert!(
        annotations
            .exclusion(name, true, Version::new(1, 18, 0))
            .is_some()
    );
    assert!(
        annotations
            .exclusion(name, true, Version::new(1, 17, 0))
            .is_none()
    );
}

#[test]
fn annotations_are_audited() {
    let cases = [
        (
            "dangling target",
            r#"[[annotations]]
            command = "aci_missing"
            source = "x"
            reason = "y"
            exclude = true"#,
        ),
        (
            "missing source",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            source = " "
            reason = "y"
            exclude = true"#,
        ),
        (
            "no fact",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            source = "x"
            reason = "y""#,
        ),
        (
            "contradiction",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            source = "x"
            reason = "y"
            params = ["Advertising_Type: u16"]"#,
        ),
        (
            "stale",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            releases = "1.18.0"
            source = "x"
            reason = "y"
            params = ["Advertising_Type: u8"]"#,
        ),
        (
            "override of unresolved",
            r#"[[annotations]]
            event = "SHCI_SUB_EVT_CODE_READY"
            source = "x"
            reason = "y"
            override = true
            payload = ["sysevt_ready_rsp: u8"]"#,
        ),
        (
            "range outside definitions",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            releases = "1.15.0..=1.19.0"
            source = "x"
            reason = "y"
            exclude = true"#,
        ),
        (
            "overlap",
            r#"[[annotations]]
            command = "aci_gap_set_discoverable"
            source = "x"
            reason = "y"
            exclude = true
            [[annotations]]
            command = "aci_gap_set_discoverable"
            releases = "1.16.0"
            source = "x"
            reason = "y"
            exclude = true"#,
        ),
    ];
    for (case, source) in cases {
        let error = audit(source).expect_err(case);
        assert!(
            matches!(error.kind(), ErrorKind::Audit | ErrorKind::Parse),
            "{case}: {error}"
        );
    }

    audit(
        r#"[[annotations]]
        command = "aci_gap_set_discoverable"
        releases = "1.18.0"
        source = "x"
        reason = "y"
        override = true
        params = ["Advertising_Type: u16"]"#,
    )
    .unwrap();
}

fn target(release: &str, profile: Profile) -> Target {
    Target {
        release: release.parse().unwrap(),
        profile,
    }
}

#[test]
fn target_view_filters_by_profile_and_release() {
    let catalog = sample();
    let annotations = Annotations::default();
    let view = |release: &str, profile| {
        TargetView::new(&catalog, &annotations, target(release, profile))
            .unwrap()
            .commands()
            .count()
    };
    assert_eq!(view("1.16.0", Profile::Full), 1);
    assert_eq!(view("1.17.0", Profile::Full), 0);
    assert_eq!(view("1.17.0", Profile::FullExtended), 1);
    assert!(TargetView::new(&catalog, &annotations, target("1.30.0", Profile::Full)).is_err());

    let view = TargetView::new(
        &catalog,
        &annotations,
        target("1.18.0", Profile::FullExtended),
    )
    .unwrap();
    assert_eq!(view.families(), [Family::Wb5x]);
    let command = view.commands().next().unwrap();
    assert_eq!(command.name, "aci_gap_set_discoverable");
    assert!(matches!(command.params.provenance, Provenance::Extracted));
    assert_eq!(command.params.fields.unwrap().len(), 1);
    assert!(command.excluded.is_none());
    let event = view.events().next().unwrap();
    assert_eq!(
        event.payload.fields,
        Err("field sysevt_ready_rsp is an enum")
    );
    let view = TargetView::new(&catalog, &annotations, target("1.18.0", Profile::Full)).unwrap();
    assert!(view.families().is_empty());
}

#[test]
fn target_view_applies_annotations() {
    let catalog = sample();
    let annotations = Annotations::from_toml(
        r#"
        [[annotations]]
        event = "SHCI_SUB_EVT_CODE_READY"
        source = "AN5289 section 4.8.1"
        reason = "the enum is transmitted as one byte"
        payload = ["sysevt_ready_rsp: u8"]

        [[annotations]]
        command = "aci_gap_set_discoverable"
        releases = "1.18.0"
        source = "x"
        reason = "y"
        exclude = true
    "#,
    )
    .unwrap();
    let view = |release| {
        TargetView::new(
            &catalog,
            &annotations,
            target(release, Profile::FullExtended),
        )
        .unwrap()
    };

    let event = view("1.15.0").events().next().unwrap();
    assert!(matches!(event.payload.provenance, Provenance::Annotated(_)));
    assert_eq!(event.payload.fields.unwrap().len(), 1);

    // Excluded entries stay visible so completeness checks can report why.
    assert!(view("1.17.0").commands().next().unwrap().excluded.is_none());
    assert!(view("1.18.0").commands().next().unwrap().excluded.is_some());

    let unaudited = Annotations::from_toml(
        r#"
        [[annotations]]
        command = "aci_missing"
        source = "x"
        reason = "y"
        exclude = true
    "#,
    )
    .unwrap();
    assert!(TargetView::new(&catalog, &unaudited, target("1.15.0", Profile::Full)).is_err());
}
