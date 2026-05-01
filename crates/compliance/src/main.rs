use anyhow::Result;

mod parse;
mod resolve;
mod source;
mod spec;

use crate::source::CubeSource;
use crate::spec::FirmwareSpec;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::parse();
    let source = CubeSource::new(args.cube, args.tag);

    let groups = [
        ("gap", "ble_gap_aci.c", "ble_gap_aci.h"),
        ("gatt", "ble_gatt_aci.c", "ble_gatt_aci.h"),
        ("hal", "ble_hal_aci.c", "ble_hal_aci.h"),
        ("hci_le", "ble_hci_le.c", "ble_hci_le.h"),
        ("l2cap", "ble_l2cap_aci.c", "ble_l2cap_aci.h"),
    ];

    let mut commands = Vec::new();
    for (group, source_name, header_name) in groups {
        let command_source = source.load_auto_file(source_name)?;
        let command_header = source.load_auto_file(header_name)?;
        commands.extend(parse::parse_group(group, &command_source, &command_header)?);
    }

    if let Some(command) = args.command {
        commands.retain(|c| c.name == command);
    }

    let types = source.load_auto_file("ble_types.h")?;
    let mut packed_structs = parse::parse_packed_structs(&types)?;
    resolve::resolve_command_payloads(&mut commands, &packed_structs);

    if let Some(struct_name) = args.struct_name {
        packed_structs.retain(|s| s.name == struct_name);
    }

    let spec = FirmwareSpec {
        firmware: source.firmware_label(),
        packed_structs,
        commands,
    };

    println!("{}", serde_json::to_string_pretty(&spec)?);
    Ok(())
}

struct Args {
    cube: PathBuf,
    tag: Option<String>,
    command: Option<String>,
    struct_name: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let mut cube = PathBuf::from("STM32CubeWB");
        let mut tag = Some("v1.15.0".to_owned());
        let mut command = None;
        let mut struct_name = None;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--cube" => {
                    if let Some(value) = args.next() {
                        cube = value.into();
                    }
                }
                "--tag" => {
                    tag = args.next();
                }
                "--worktree" => tag = None,
                "--command" => command = args.next(),
                "--struct" => struct_name = args.next(),
                _ => {}
            }
        }

        Self {
            cube,
            tag,
            command,
            struct_name,
        }
    }
}
