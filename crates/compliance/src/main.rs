use anyhow::Result;
use clap::{Parser, Subcommand};

mod check;
mod diff;
mod parse;
mod resolve;
mod source;
mod spec;

use crate::check::check_coverage;
use crate::diff::diff_firmware;
use crate::source::CubeSource;
use crate::spec::FirmwareSpec;
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.subcommand {
        Some(Command::Diff { from, to }) => {
            let from = build_spec(&cli.cube, Some(from.clone()), None, None)?;
            let to = build_spec(&cli.cube, Some(to.clone()), None, None)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&diff_firmware(&from, &to))?
            );
        }
        Some(Command::Check { rust_crate }) => {
            let tag = (!cli.worktree).then(|| cli.tag.clone());
            let spec = build_spec(&cli.cube, tag, None, None)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&check_coverage(&spec, rust_crate)?)?
            );
        }
        None | Some(Command::Extract) => {
            let tag = (!cli.worktree).then(|| cli.tag.clone());
            let spec = build_spec(&cli.cube, tag, cli.command.clone(), cli.struct_name.clone())?;
            println!("{}", serde_json::to_string_pretty(&spec)?);
        }
    }
    Ok(())
}

fn build_spec(
    cube: &PathBuf,
    tag: Option<String>,
    command_filter: Option<String>,
    struct_filter: Option<String>,
) -> Result<FirmwareSpec> {
    let source = CubeSource::new(cube, tag);
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

    if let Some(command) = command_filter {
        commands.retain(|c| c.name == command);
    }

    let types = source.load_auto_file("ble_types.h")?;
    let mut packed_structs = parse::parse_packed_structs(&types)?;
    resolve::resolve_command_payloads(&mut commands, &packed_structs);
    resolve::resolve_command_return_payloads(&mut commands, &packed_structs);
    let events = parse::parse_events(&source.load_auto_file("ble_events.h")?);

    if let Some(struct_name) = struct_filter {
        packed_structs.retain(|s| s.name == struct_name);
    }

    Ok(FirmwareSpec {
        firmware: source.firmware_label(),
        packed_structs,
        commands,
        events,
    })
}

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, default_value = "STM32CubeWB", global = true)]
    cube: PathBuf,

    #[arg(long, default_value = "v1.15.0", global = true)]
    tag: String,

    #[arg(long, global = true)]
    worktree: bool,

    #[arg(long, global = true)]
    command: Option<String>,

    #[arg(long = "struct", global = true)]
    struct_name: Option<String>,

    #[command(subcommand)]
    subcommand: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Extract,
    Diff {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
    },
    Check {
        #[arg(long, default_value = "crates/stm32wb-hci")]
        rust_crate: PathBuf,
    },
}
