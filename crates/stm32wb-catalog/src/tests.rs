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
        bearers: Vec::new(),
        domains: Vec::new(),
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
            profiles: [Platform::Stm32wb.profiles(), &[Profile::Full]].concat(),
            payload: Layout::Unresolved("field sysevt_ready_rsp is an enum".into()),
            structs: Structs::new(),
            bearers: Vec::new(),
            domains: Vec::new(),
        }],
        statuses: Vec::new(),
        struct_domains: Vec::new(),
    }
}

#[test]
fn structure_fields_have_their_own_domains() {
    let address_types = |last: i64| Domain {
        kind: DomainKind::Values,
        items: (0..=last)
            .map(|value| DomainItem {
                first: value,
                last: value,
                hex_digits: Some(2),
                label: None,
            })
            .collect(),
        unit_us: None,
    };
    let with = |version: &str, last: i64| {
        let mut command = set_discoverable(
            &[Profile::FullExtended],
            fields(&["Num: u8", "Peer: [Peer_Entry_t; Num] (capacity 2)"]),
        );
        command.structs = Structs::from([(
            "Peer_Entry_t".to_owned(),
            vec!["Peer_Address_Type: u8".parse().unwrap()],
        )]);
        let mut snapshot = snapshot(version, vec![command]);
        snapshot.struct_domains = vec![(
            "Peer_Entry_t".to_owned(),
            "Peer_Address_Type".to_owned(),
            address_types(last),
        )];
        snapshot
    };
    let merged = merge_snapshots(
        Platform::Stm32wb,
        vec![with("1.15.0", 1), with("1.16.0", 1), with("1.17.0", 3)],
    )
    .unwrap();
    let rendered = merged
        .struct_domains
        .iter()
        .map(|domain| {
            format!(
                "{} {}.{} {}",
                domain.releases,
                domain.structure,
                domain.member,
                domain.domain.items.len()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered,
        [
            "1.15.0..=1.16.0 Peer_Entry_t.Peer_Address_Type 2",
            "1.17.0 Peer_Entry_t.Peer_Address_Type 4"
        ]
    );
    let release = "1.17.0".parse().unwrap();
    assert_eq!(
        merged
            .struct_domain("Peer_Entry_t", "Peer_Address_Type", release)
            .map(|domain| domain.items.len()),
        Some(4)
    );
    assert!(Catalog::from_toml(&merged.to_toml().unwrap()).unwrap() == merged);

    let mut uncarried = with("1.15.0", 1);
    uncarried.commands[0].structs.clear();
    uncarried.commands[0].params = fields(&["Num: u8"]);
    let error = merge_snapshots(Platform::Stm32wb, vec![uncarried]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("no command or event of 1.15.0 carries"),
        "{error}"
    );
    let error = merge_snapshots(Platform::Stm32wb, vec![with("1.15.0", 0x100)]).unwrap_err();
    assert!(error.to_string().contains("cannot hold"), "{error}");
}

#[test]
fn statuses_have_their_own_histories() {
    let with = |version: &str, statuses: &[(&str, u8)]| {
        let mut snapshot = snapshot(version, Vec::new());
        snapshot.statuses = statuses
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect();
        snapshot
    };
    let failed = ("BLE_STATUS_FAILED", 0x91);
    let full = ("BLE_STATUS_SEC_DB_FULL", 0x5D);
    let merged = merge_snapshots(
        Platform::Stm32wb,
        vec![
            with("1.15.0", &[failed, full]),
            with("1.16.0", &[full, failed]),
            with("1.17.0", &[failed]),
        ],
    )
    .unwrap();
    let rendered = merged
        .statuses
        .iter()
        .map(|status| format!("{} {}=0x{:02X}", status.releases, status.name, status.value))
        .collect::<Vec<_>>();
    assert_eq!(
        rendered,
        [
            "1.15.0..=1.16.0 BLE_STATUS_SEC_DB_FULL=0x5D",
            "1.15.0..=1.17.0 BLE_STATUS_FAILED=0x91"
        ]
    );
    let values = |release: &str| {
        merged
            .statuses_at(release.parse().unwrap())
            .iter()
            .map(|status| status.value)
            .collect::<Vec<_>>()
    };
    assert_eq!(values("1.16.0"), [0x5D, 0x91]);
    assert_eq!(values("1.17.0"), [0x91]);
    assert!(Catalog::from_toml(&merged.to_toml().unwrap()).unwrap() == merged);

    let twice = merge_snapshots(
        Platform::Stm32wb,
        vec![with("1.15.0", &[failed, ("BLE_STATUS_ERROR", 0x91)])],
    );
    assert!(twice.unwrap_err().to_string().contains("defined twice"));
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
fn domains_have_their_own_histories() {
    let layout = fields(&["Advertising_Type: u8", "Interval: u16"]);
    let types = |items: &[&str]| Domain {
        kind: DomainKind::Values,
        items: items.iter().map(|item| item.parse().unwrap()).collect(),
        unit_us: None,
    };
    let interval = Domain {
        kind: DomainKind::Values,
        items: vec!["0x0020..=0x4000".parse().unwrap()],
        unit_us: Some(625),
    };
    let documented = |items: &[&str]| {
        let mut command = set_discoverable(&[Profile::FullExtended], layout.clone());
        command.domains = vec![
            ("Advertising_Type".into(), false, types(items)),
            ("Interval".into(), false, interval.clone()),
        ];
        command
    };
    let merged = merge_snapshots(
        Platform::Stm32wb,
        vec![
            snapshot("1.15.0", vec![documented(&["0x00: ADV_IND"])]),
            snapshot("1.16.0", vec![documented(&["0x00: ADV_IND"])]),
            snapshot(
                "1.17.0",
                vec![documented(&["0x00: ADV_IND", "0x02: ADV_SCAN_IND"])],
            ),
        ],
    )
    .unwrap();
    let command = &merged.commands[0];
    // A relabelled list splits the member's history, not the definition.
    assert_eq!(command.definitions.len(), 1);
    let ranges = command
        .domains
        .iter()
        .map(|domain| format!("{} {}", domain.member, domain.releases))
        .collect::<Vec<_>>();
    assert_eq!(
        ranges,
        [
            "Advertising_Type 1.15.0..=1.16.0",
            "Advertising_Type 1.17.0",
            "Interval 1.15.0..=1.17.0",
        ]
    );
    let reparsed = Catalog::from_toml(&merged.to_toml().unwrap()).unwrap();
    assert_eq!(reparsed, merged);

    let mut unknown = documented(&["0x00: ADV_IND"]);
    unknown.domains[0].0 = "Missing".into();
    let mut too_wide = documented(&["0x00: ADV_IND"]);
    too_wide.domains[0].2 = types(&["0x100: Too wide"]);
    let unordered = documented(&["0x02: ADV_SCAN_IND", "0x00: ADV_IND"]);
    let mut returned = documented(&["0x00: ADV_IND"]);
    returned.domains[0].1 = true;
    let mut flags = documented(&["0x00: None", "0x03: Two bits"]);
    flags.domains[0].2.kind = DomainKind::Flags;
    for (command, expected) in [
        (unknown, "is not a member"),
        (too_wide, "which a u8 cannot hold"),
        (unordered, "out of order"),
        (returned, "is not a member"),
        (flags, "not a single bit"),
    ] {
        let error = merge_snapshots(Platform::Stm32wb, vec![snapshot("1.15.0", vec![command])])
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn array_domains_constrain_each_element() {
    let layout = fields(&[
        "Num_Subevents: u8",
        "Subevent: [u8; Num_Subevents] (capacity 10)",
        "Link_Status: [u8; 22]",
    ]);
    let documented = |member: &str, item: &str| {
        let mut command = set_discoverable(&[Profile::FullExtended], layout.clone());
        command.domains = vec![(
            member.into(),
            false,
            Domain {
                kind: DomainKind::Values,
                items: vec![item.parse().unwrap()],
                unit_us: None,
            },
        )];
        vec![snapshot("1.15.0", vec![command])]
    };
    for member in ["Subevent", "Link_Status"] {
        merge_snapshots(Platform::Stm32wb, documented(member, "0x00..=0x7F")).unwrap();
        let error =
            merge_snapshots(Platform::Stm32wb, documented(member, "0x100: Too wide")).unwrap_err();
        assert!(
            error.to_string().contains("which a u8 element cannot hold"),
            "{error}"
        );
    }
}

#[test]
fn domain_items_round_trip_as_documented() {
    for source in [
        "0x00: ADV_IND (Connectable undirected advertising)",
        "0x0020..=0x4000",
        "-127..=20: Tx power",
        "0xFFFF: No specific minimum",
        "23..=65535",
        "0x0000000000000000: No events specified",
    ] {
        let item: DomainItem = source.parse().unwrap();
        assert_eq!(item.to_string(), source);
    }
    for source in [
        "0x20..=10",
        "0x0a",
        "5..=1",
        "0x10..=0x0F",
        "x: y",
        "0x00:label",
    ] {
        assert!(source.parse::<DomainItem>().is_err(), "{source}");
    }
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

#[test]
fn bundled_catalog_is_valid_and_audited() {
    let Bundled {
        catalog,
        annotations,
    } = bundled().unwrap();
    assert_eq!(catalog.platform, Platform::Stm32wb);
    assert_eq!(
        catalog.releases.first().unwrap().version,
        Version::new(1, 15, 0)
    );
    assert!(catalog.command_named("aci_gap_set_discoverable").is_some());
    assert!(!annotations.annotations.is_empty());
    assert!(std::ptr::eq(catalog, &bundled().unwrap().catalog));
}

#[test]
fn bundled_system_commands_keep_their_history() {
    let bundled = bundled().unwrap();
    let init = bundled
        .catalog
        .command(CommandScope::System, 0xFC66)
        .unwrap();
    assert_eq!(init.name(), "SHCI_C2_BLE_Init");
    assert_eq!(
        init.definitions
            .iter()
            .map(|definition| definition.releases.to_string())
            .collect::<Vec<_>>(),
        [
            "1.15.0",
            "1.16.0..=1.17.3",
            "1.18.0..=1.22.1",
            "1.23.0",
            "1.24.0"
        ]
    );
    let segments = bundled.command_segments("SHCI_C2_THREAD_Init").unwrap();
    assert!(
        segments
            .iter()
            .all(|segment| segment.entry.excluded.is_some())
    );
    let segments = bundled.command_segments("SHCI_C2_FUS_GetState").unwrap();
    let returns = segments[0].entry.returns.unwrap().fields.unwrap();
    assert_eq!(returns.len(), 2);
}

#[test]
fn bearers_are_written_with_their_range() {
    let bearer = "Connection_Handle: 0xEA00..=0xEA3F"
        .parse::<Bearer>()
        .unwrap();
    assert_eq!(
        bearer,
        Bearer {
            member: "Connection_Handle".into(),
            first: 0xEA00,
            last: 0xEA3F,
        }
    );
    assert_eq!(bearer.to_string(), "Connection_Handle: 0xEA00..=0xEA3F");
    for malformed in [
        "Connection_Handle",
        "Connection_Handle: 0xea00..=0xea3f",
        "Connection_Handle: 0xEA00...0xEA3F",
    ] {
        assert!(malformed.parse::<Bearer>().is_err(), "{malformed}");
    }
}

#[test]
fn validation_checks_bearer_members() {
    let params = fields(&["Connection_Handle: u16", "Attr_Handle: u16", "Offset: u8"]);
    let check = |bearer: &str| validate_bearers(&params, &[bearer.parse().unwrap()]);
    assert!(check("Connection_Handle: 0xEA00..=0xEA3F").is_ok());
    let error = check("Offset: 0xEA00..=0xEA3F").unwrap_err().to_string();
    assert!(error.contains("Offset is not a u16 member"), "{error}");
    let error = check("Missing: 0xEA00..=0xEA3F").unwrap_err().to_string();
    assert!(error.contains("Missing is not a u16 member"), "{error}");
    let error = check("Connection_Handle: 0xEA10..=0xEA3F")
        .unwrap_err()
        .to_string();
    assert!(error.contains("not 0xEA00 and channel indexes"), "{error}");
    let error = check("Connection_Handle: 0xEA00..=0xEB00")
        .unwrap_err()
        .to_string();
    assert!(error.contains("not 0xEA00 and channel indexes"), "{error}");

    let bearer = "Connection_Handle: 0xEA00..=0xEA1F"
        .parse::<Bearer>()
        .unwrap();
    let twice = [bearer.clone(), bearer];
    let error = validate_bearers(&params, &twice).unwrap_err().to_string();
    assert!(error.contains("listed twice"), "{error}");
    let unresolved = Layout::Unresolved("procedural".into());
    assert!(validate_bearers(&unresolved, &twice[..1]).is_err());
}

/// The GATT client took enhanced ATT bearers on 32 channels, then 64 from
/// 1.17.0; the server side confirms and notifies on them from 1.16.0.
#[test]
fn bundled_bearers_follow_the_documentation() {
    let catalog = &bundled().unwrap().catalog;
    let bearers = |name: &str, release: Version| {
        catalog
            .command_named(name)
            .unwrap()
            .definition_at(release)
            .unwrap()
            .bearers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    let (v1_15, v1_16, v1_24) = (
        Version::new(1, 15, 0),
        Version::new(1, 16, 0),
        Version::new(1, 24, 0),
    );
    assert_eq!(
        bearers("aci_gatt_read_char_value", v1_15),
        ["Connection_Handle: 0xEA00..=0xEA1F"]
    );
    assert_eq!(
        bearers("aci_gatt_read_char_value", v1_24),
        ["Connection_Handle: 0xEA00..=0xEA3F"]
    );
    assert!(bearers("aci_gatt_confirm_indication", v1_15).is_empty());
    assert_eq!(
        bearers("aci_gatt_confirm_indication", v1_16),
        ["Connection_Handle: 0xEA00..=0xEA1F"]
    );
    assert_eq!(
        bearers("aci_gatt_update_char_value_ext", v1_24),
        ["Conn_Handle_To_Notify: 0xEA00..=0xEA3F"]
    );
    assert!(bearers("aci_gap_terminate", v1_24).is_empty());

    let event = catalog.event_named("aci_gatt_notification_event").unwrap();
    assert!(event.definition_at(v1_15).unwrap().bearers.is_empty());
    assert_eq!(event.definition_at(v1_24).unwrap().bearers.len(), 1);
}

fn ranges<T>(segments: &[Segment<'_, T>]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| segment.releases.to_string())
        .collect()
}

#[test]
fn segments_split_where_any_fact_changes() {
    let bundled = Bundled::new(sample(), Annotations::default()).unwrap();
    let segments = bundled
        .command_segments("aci_gap_set_discoverable")
        .unwrap();
    // Availability changes in 1.17.0, the definition in 1.18.0.
    assert_eq!(ranges(&segments), ["1.15.0..=1.16.0", "1.17.0", "1.18.0"]);
    assert_eq!(segments[0].profiles, [Profile::FullExtended, Profile::Full]);
    assert_eq!(segments[1].profiles, [Profile::FullExtended]);
    assert!(
        segments
            .iter()
            .all(|segment| segment.entry.excluded.is_none())
    );

    let annotations = Annotations::from_toml(
        r#"
        [[annotations]]
        command = "aci_gap_set_discoverable"
        releases = "1.16.0"
        source = "x"
        reason = "y"
        exclude = true

        [[annotations]]
        event = "SHCI_SUB_EVT_CODE_READY"
        source = "x"
        reason = "y"
        payload = ["sysevt_ready_rsp: u8"]
    "#,
    )
    .unwrap();
    let bundled = Bundled::new(sample(), annotations).unwrap();
    let segments = bundled
        .command_segments("aci_gap_set_discoverable")
        .unwrap();
    assert_eq!(ranges(&segments), ["1.15.0", "1.16.0", "1.17.0", "1.18.0"]);
    assert!(segments[1].entry.excluded.is_some());

    let segments = bundled.event_segments("SHCI_SUB_EVT_CODE_READY").unwrap();
    assert_eq!(ranges(&segments), ["1.15.0..=1.18.0"]);
    assert!(matches!(
        segments[0].entry.payload.provenance,
        Provenance::Annotated(_)
    ));

    assert!(bundled.command_segments("aci_missing").is_err());
    assert!(bundled.event_segments("aci_gap_set_discoverable").is_err());
}

fn with_opcode(opcode: u16, name: &str) -> SnapshotCommand {
    SnapshotCommand {
        opcode,
        name: name.into(),
        ..set_discoverable(&[Profile::FullExtended], fields(&["Mode: u8"]))
    }
}

fn history(snapshots: Vec<Snapshot>) -> Bundled {
    let catalog = merge_snapshots(Platform::Stm32wb, snapshots).unwrap();
    Bundled::new(catalog, Annotations::default()).unwrap()
}

#[test]
fn segments_follow_names_across_gaps_renames_and_moves() {
    let bundled = history(vec![
        snapshot("1.15.0", vec![with_opcode(0xFC83, "aci_old")]),
        snapshot("1.16.0", vec![]),
        snapshot("1.17.0", vec![with_opcode(0xFC83, "aci_new")]),
        snapshot("1.18.0", vec![with_opcode(0xFC84, "aci_new")]),
    ]);
    let segments = bundled.command_segments("aci_new").unwrap();
    assert_eq!(ranges(&segments), ["1.15.0", "1.17.0", "1.18.0"]);
    let identity = segments
        .iter()
        .map(|segment| (segment.entry.command.opcode, segment.entry.name))
        .collect::<Vec<_>>();
    assert_eq!(
        identity,
        [
            (0xFC83, "aci_old"),
            (0xFC83, "aci_new"),
            (0xFC84, "aci_new")
        ]
    );
    // The renamed command's history is the same whichever name finds it.
    assert_eq!(
        ranges(&bundled.command_segments("aci_old").unwrap()),
        ["1.15.0", "1.17.0"]
    );

    let unchanged = history(vec![
        snapshot("1.15.0", vec![with_opcode(0xFC83, "aci_same")]),
        snapshot("1.16.0", vec![]),
        snapshot("1.17.0", vec![with_opcode(0xFC83, "aci_same")]),
    ]);
    assert_eq!(
        ranges(&unchanged.command_segments("aci_same").unwrap()),
        ["1.15.0", "1.17.0"]
    );

    let ambiguous = history(vec![snapshot(
        "1.15.0",
        vec![
            with_opcode(0xFC83, "aci_twice"),
            with_opcode(0xFC84, "aci_twice"),
        ],
    )]);
    let error = ambiguous.command_segments("aci_twice").unwrap_err();
    assert!(error.message().contains("2 entries in 1.15.0"), "{error}");
}

#[test]
fn bundled_segments_cover_moved_codes() {
    let bundled = bundled().unwrap();
    let segments = bundled
        .event_segments("aci_hal_end_of_radio_activity_event")
        .unwrap();
    let first = segments.first().unwrap();
    let last = segments.last().unwrap();
    assert_eq!(first.releases.first, Version::new(1, 15, 0));
    assert_eq!(last.releases.last, Version::new(1, 24, 0));
    assert_ne!(first.entry.event.code, last.entry.event.code);

    let segments = bundled
        .command_segments("aci_gap_peripheral_security_req")
        .unwrap();
    assert_eq!(segments[0].entry.name, "aci_gap_slave_security_req");
}

#[test]
fn distinct_targets_cover_every_interface_once() {
    let sample = Bundled::new(sample(), Annotations::default()).unwrap();
    let targets = sample
        .distinct_targets()
        .into_iter()
        .map(Target::features)
        .collect::<Vec<_>>();
    // Profiles with the command see it under the first definition until
    // 1.18.0, whether or not `full` still supports it; profiles without it
    // only ever see the READY event.
    assert_eq!(
        targets,
        [
            "fw_1_15_0,stack-full-extended",
            "fw_1_15_0,stack-light",
            "fw_1_18_0,stack-full-extended",
        ]
    );

    let bundled = bundled().unwrap();
    let targets = bundled.distinct_targets();
    let oldest = bundled.catalog.versions().next().unwrap();
    assert_eq!(
        targets[0],
        Target {
            release: oldest,
            profile: Profile::FullExtended
        }
    );
    assert!(
        targets.len() < bundled.catalog.releases.len() * bundled.catalog.platform.profiles().len()
    );
}

#[test]
fn stm32wb_items_follow_the_mcu_and_profile_conditions() {
    let domain = |kind, items: &[&str]| Domain {
        kind,
        items: items.iter().map(|item| item.parse().unwrap()).collect(),
        unit_us: None,
    };
    let values = domain(
        DomainKind::Values,
        &[
            "0x00: 1M PHY",
            "0x01: Coded PHY [not supported on STM32WB]",
            "0x02: Periodic advertising [only for STM32WBA]",
            "0x03: Scan channel map (only for STM32WB)",
            "0x0004..=0x5DC0: extended advertising with STM32WB",
            "0x0004..=0xFFFF: extended advertising with STM32WBA",
            "0x5DC1: Max data length (only for STM32WB full stack)",
            "0x5DC2: Max data length [only for full stack]",
        ],
    );
    assert_eq!(
        values.stm32wb_ranges(Profile::Light).unwrap(),
        [(0, 0), (3, 3), (4, 0x5DC0)]
    );
    assert_eq!(
        values.stm32wb_ranges(Profile::Full).unwrap(),
        [
            (0, 0),
            (3, 3),
            (4, 0x5DC0),
            (0x5DC1, 0x5DC1),
            (0x5DC2, 0x5DC2)
        ]
    );
    let variant = domain(
        DomainKind::Values,
        &["0x00..=0x25: for BO variant", "0x00..=0xFF: otherwise"],
    );
    assert_eq!(
        variant.stm32wb_ranges(Profile::HciAdvScan).unwrap(),
        [(0, 0x25)]
    );
    assert_eq!(
        variant.stm32wb_ranges(Profile::HciLayer).unwrap(),
        [(0, 0xFF)]
    );
    let named = domain(
        DomainKind::Values,
        &["0x0004..=0x5DC0: STM32WB", "0x0004..=0xFFFF: STM32WBA"],
    );
    assert_eq!(named.stm32wb_ranges(Profile::Full).unwrap(), [(4, 0x5DC0)]);
    let error = domain(DomainKind::Values, &["0x0004..=0x5DC0: STM32WBx"])
        .stm32wb_ranges(Profile::Full)
        .unwrap_err();
    assert!(error.contains("cannot interpret"), "{error}");
    let error = domain(DomainKind::Values, &["0x00..=0x25: for XY variant"])
        .stm32wb_ranges(Profile::Full)
        .unwrap_err();
    assert!(error.contains("cannot interpret"), "{error}");
    let flags = domain(
        DomainKind::Flags,
        &[
            "0x00: No events",
            "0x01: Scan request report (only for STM32WB)",
            "0x02: Sync (only for STM32WBA)",
            "0x04: Coded PHY [not supported on STM32WB]",
            "0x08: Decision filter bit (not supported)",
            "0x10: Peripheral",
        ],
    );
    assert_eq!(flags.stm32wb_bits(Profile::Full).unwrap(), 0x11);
    assert!(
        flags
            .stm32wb_ranges(Profile::Full)
            .unwrap_err()
            .contains("bits")
    );
    assert!(
        values
            .stm32wb_bits(Profile::Full)
            .unwrap_err()
            .contains("values")
    );
    assert_eq!(
        domain(DomainKind::Values, &["0x01: Resolving (not supported)"])
            .stm32wb_ranges(Profile::Full)
            .unwrap(),
        []
    );
}

#[test]
fn stm32wb_lengths_end_the_labels() {
    let offsets = Domain {
        kind: DomainKind::Values,
        items: [
            "0x00: CONFIG_DATA_PUBLIC_ADDRESS_OFFSET; Bluetooth public address; 6 bytes",
            "0x34: CONFIG_DATA_GAP_ADD_REC_NBR_OFFSET; GAP service additional record number",
            "0xB0: CONFIG_DATA_SMP_MODE_OFFSET; SMP mode; 1 byte",
            "0xC2: CONFIG_DATA_LL_RSSI_GOLDEN_RANGE_OFFSET [only for STM32WBA]; LL RSSI golden range; 2 bytes",
            "0xD0..=0xD1: Reserved; 2 bytes",
        ]
        .iter()
        .map(|item| item.parse().unwrap())
        .collect(),
        unit_us: None,
    };
    assert_eq!(
        offsets.stm32wb_lengths(Profile::Light).unwrap(),
        [(0x00, 6), (0xB0, 1)]
    );
    let item: DomainItem = "0xD1: Max data length (bytes #0-1: \"tx\"); 8 bytes"
        .parse()
        .unwrap();
    assert_eq!(item.length(), Some(8));
    let flags = Domain {
        kind: DomainKind::Flags,
        ..offsets
    };
    assert!(
        flags
            .stm32wb_lengths(Profile::Light)
            .unwrap_err()
            .contains("bits")
    );
}
