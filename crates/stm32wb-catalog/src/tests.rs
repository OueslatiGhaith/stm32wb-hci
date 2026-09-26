use super::*;

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
