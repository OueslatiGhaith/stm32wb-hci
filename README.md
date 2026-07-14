# STM32WB-HCI

forked from [bluetooth_hci](https://github.com/danielgallagher0/bluetooth-hci)

[![Build Status](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)

This crate defines a pure Rust implementation of the [Bluetooth Host-Controller Interface](https://github.com/STMicroelectronics/STM32CubeWB/) for the STM32WB family of microcontrollers. It defines commands
and events from the specification, and vendor-specific commands and events.

## Firmware selection

One crate release can target several STM32WB wireless-firmware versions. Select exactly one
firmware feature; `fw_0_17_1` is the default. The feature names retain this crate's historical
`0.x.y` compatibility notation, while the corresponding STM32CubeWB tags use `v1.x.y`.

The `[features]` table in `Cargo.toml` is the source of truth. To see the complete, current set
without relying on this documentation table, run:

```sh
cargo run -p stm32wb-compliance -- list-supported
```

| Cargo feature | STM32CubeWB tag |
| --- | --- |
| `fw_0_15_0` | `v1.15.0` |
| `fw_0_16_0` | `v1.16.0` |
| `fw_0_17_0` | `v1.17.0` |
| `fw_0_17_1` | `v1.17.1` |

For a non-default firmware, disable default features before selecting it:

```sh
cargo build --no-default-features --features fw_0_15_0
```

The build script makes these internal conditional-compilation predicates available to the crate:
`before_fw_0_17_0`, `only_fw_0_17_0`, and `since_fw_0_17_0`. `before` is a strict comparison;
`since` includes the named firmware. Do not use
`--all-features`, because firmware features are intentionally mutually exclusive. The shorter
`before_0_17_0`/`only_0_17_0`/`since_0_17_0` spellings are emitted as compatibility
aliases.

## Firmware compliance

The `stm32wb-compliance` workspace tool compares the selected feature surface against the matching
tag in a local STM32CubeWB clone. It reads tagged blobs with `git show`, so running it does not
checkout, modify, or otherwise disturb `STM32CubeWB`.

```sh
# STM32CubeWB is expected at ./STM32CubeWB by default.
cargo run -p stm32wb-compliance -- check --firmware 0.15.0 --deny

# Discover every fw_* feature in Cargo.toml and check all of them.
cargo run -p stm32wb-compliance -- check --all-supported --deny
```

`--deny` makes differences or unavailable wire evidence fail the command for CI; omit it to
inspect the report, or pass `--json` for machine-readable output. `--all-supported` discovers
canonical `fw_<major>_<minor>_<patch>` entries directly from the crate's `[features]` table, so
adding a firmware feature automatically puts it in the compliance and CI loops.

The checker reports the requested CubeWB tag, the tag object, and the resolved commit, making a
result reproducible even when inspecting a local CubeWB clone. Its exclusions are governed by the
checked-in [policy](tools/compliance/exclusions.policy). The policy requires a reason for every
exception and rejects malformed, overlapping, unsupported-version, or stale entries; an exception
must actively suppress a real coverage difference. Transport-only events can additionally declare
a fixed or bounded payload envelope, which is checked against the Rust event schema. Pass
`--policy <path>` only when evaluating a deliberate alternative policy.

The checker parses both the Rust crate and CubeWB's generated C sources as syntax trees, and
evaluates the selected firmware cfgs, so module-, trait-, impl-, method-, and branch-level gates
are all included in the active API inventory. It compares:

- vendor ACI command IDs and vendor-event IDs from CubeWB's generated `ble_*_aci.c` and
  `ble_events.c` files;
- standard HCI command opcodes, ordinary events, and LE Meta subevents from `ble_hci_le.c` and
  `ble_events.c` against the crate's public `bt_hci` re-export plus STM32WB command extensions;
- vendor command and event payload envelopes: exact and bounded request, return, and event sizes,
  plus Command Complete versus Command Status completion. Command returns are normalized without
  the transport status byte, matching the declarative `Return` schema.

CubeWB request-length formulas are normalized from their generated C parameter types, local
branches, packed `sizeof` types, and the 255-byte HCI limit. Capacity-shaped command returns are
normalized from their packed structures and `BLE_EVT_MAX_PARAM_LEN` expressions. Unsupported
expressions or C layouts are reported as `wire unavailable` rather than guessed and make the
report non-compliant. All transport-only exceptions—including the coprocessor-ready event
`0x9200`—come exclusively from the checked-in policy; its one-byte payload envelope is validated
like generated event metadata, and library defaults do not hide it.

## Usage

This crate works with controllers that implement `bt_hci::controller::Controller` and the
proprietary ST HCI specification. Standard commands and events come directly from the public
`bt_hci` re-export. Vendor commands are generated command types: construct one, then call
`SyncCmd::exec` for a Command Complete command or `AsyncCmd::exec` for a Command Status command.
The adapter executes both standard and vendor types through `ControllerCmdSync` and
`ControllerCmdAsync`.

The controller's `read` method may have to be polled for commands to complete. A channel or other
method may be used so that packet reads remain active while commands execute.

```rust
    use bt_hci::{
        ControllerToHostPacket,
        cmd::{SyncCmd, controller_baseband::Reset},
        controller::Controller as _,
        event::EventKind,
        param::BdAddr,
    };
    use stm32wb_hci::vendor::{
        command::{
            gap::{CmdGapInit, Role},
            gatt::GattInit,
            hal::HalWriteConfigData,
        },
        event::VendorEvent,
    };

    let ble = ControllerAdapter::new(ble);

    join(
        async {
            let mut packet_buffer = [0; 260];
            loop {
                match ble.read(&mut packet_buffer).await {
                    Ok(ControllerToHostPacket::Event(event))
                        if event.kind == EventKind::Vendor =>
                    {
                        let event = VendorEvent::new(event.data)
                            .expect("valid STM32WB vendor event");
                        defmt::info!("vendor event: {}", event);
                    }
                    Ok(packet) => defmt::info!("packet: {}", packet),
                    Err(_) => defmt::error!("failed to read HCI packet"),
                }
            }
        },
        async {
            let response = Reset::new().exec(&ble).await;
            defmt::info!("{}", response);

            let public_address = BdAddr([0xE7, 0xCA, 0x10, 0x01, 0x00, 0xE1]);
            let command = HalWriteConfigData::try_new(0, &public_address.0)
                .expect("a public address fits the command payload");
            let response = command.exec(&ble).await;
            defmt::info!("{}", response);

            let response = GattInit::new().exec(&ble).await;
            defmt::info!("{}", response);

            let response = CmdGapInit::new(Role::PERIPHERAL, false, 8)
                .exec(&ble)
                .await;
            defmt::info!("{}", response);

            info!("BLE HCI ready");
        },
    )
    .await;
```
