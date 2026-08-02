# STM32WB-HCI

forked from [bluetooth_hci](https://github.com/danielgallagher0/bluetooth-hci)

[![Build Status](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)

This crate defines a pure Rust implementation of the [Bluetooth Host-Controller Interface](https://github.com/STMicroelectronics/STM32CubeWB/) for the STM32WB family of microcontrollers. It defines commands
and events from the specification, and vendor-specific commands and events.

## Cube release and wireless binary selection

One crate release can target several STM32CubeWB protocol releases. Select exactly one `fw_*`
feature; `fw_1_24_0` is the default.

The `[features]` table in `Cargo.toml` is the source of truth. To see the complete, current set, run:

```sh
cargo run -p stm32wb-compliance -- list-supported
```

For a non-default firmware, disable default features before selecting it:

```sh
cargo build --no-default-features --features fw_1_15_0
```

The build script makes these internal conditional-compilation predicates available to the crate:
`before_fw_1_17_0`, `only_fw_1_17_0`, and `since_fw_1_17_0`. `before` is a strict comparison;
`since` includes the named firmware. Do not use
`--all-features`, because firmware features are intentionally mutually exclusive.

## Wireless-binary compliance

The `stm32wb-compliance` workspace tool compares the selected feature surface against an explicit
`{Cube release, MCU family, stack profile}` target in a local STM32CubeWB clone. It reads tagged
blobs with `git show`, so running it does not checkout, modify, or otherwise disturb
`STM32CubeWB`.

The generated C wrappers describe Cube's complete shared host interface. Binary membership is
resolved separately from the same tag: `STM32WB_BLE_Wireless_Interface.html` supplies the
BF/PO/LO/LB/BO command and event availability tables, and the selected family's wireless-binary
release notes map those profiles to exact `.bin` names. The default target is the WB5x
`BLE_Stack_full_extended` binary, for which Cube documents the complete interface.

The compliance tool's [guide](crates/stm32wb-compliance/README.md) documents its
internal normalized catalog, JSON report, and TOML exclusion policy.

```sh
# STM32CubeWB is expected at ./STM32CubeWB by default.
cargo run -p stm32wb-compliance -- check --release 1.15.0 \
  --family wb5x --profile full-extended --deny

# Discover every fw_* feature in Cargo.toml and check all of them.
cargo run -p stm32wb-compliance -- check --all-supported --deny
```

`--deny` makes differences or unavailable wire evidence fail the command for CI; omit it to
inspect the report, or pass `--json` for machine-readable output. `--all-supported` discovers
canonical `fw_<major>_<minor>_<patch>` entries directly from the crate's `[features]` table, so
adding a firmware feature automatically puts it in the compliance and CI loops.

The checker reports the requested CubeWB tag, the tag object, resolved commit, exact binary path,
and binary Git blob, making a result reproducible even when inspecting a local CubeWB clone. Its exclusions are governed by the
checked-in [TOML policy](crates/stm32wb-compliance/exclusions.toml). The policy requires a reason
for every exception and rejects malformed, overlapping, unsupported-version, or stale entries; an
exception must actively suppress a real coverage difference. System-channel events must be present
in a tagged CubeWB source inventory, and their payload lengths are checked against the Rust event
schema. Pass `--policy <path>` only when evaluating a deliberate alternative policy.

The checker parses both the Rust crate and CubeWB's generated C sources as syntax trees, and
evaluates the selected firmware cfgs, so module-, trait-, impl-, method-, and branch-level gates
are all included in the active API inventory. It compares:

- vendor ACI command IDs and vendor-event IDs from CubeWB's generated `ble_*_aci.c` and
  `ble_events.c` files, plus system-channel event IDs from the tagged `shci.h` header;
- standard HCI command opcodes declared directly in `src/standard.rs` against the selected
  CubeWB catalog; APIs inherited from `bt-hci` are intentionally outside compliance scope;
- vendor command and event payload envelopes: exact and bounded request, return, and event sizes,
  plus Command Complete versus Command Status completion. Command returns are normalized without
  the transport status byte, matching the declarative `Return` schema.

CubeWB request-length formulas are normalized from their generated C parameter types, local
branches, packed `sizeof` types, and the 255-byte HCI limit. Capacity-shaped command returns are
normalized from their packed structures and `BLE_EVT_MAX_PARAM_LEN` expressions. Unsupported
expressions or C layouts are reported as `wire unavailable` rather than guessed and make the
report non-compliant. SHCI system events—including coprocessor-ready, error notification, and BLE
NVM updates—are source-backed catalog entries, so an event missing from both `ble_events.c` and the
Rust API can no longer disappear from the comparison. The checked-in policy explicitly excludes
only the OpenThread NVM and concurrent 802.15.4 notifications.

CubeWB conditional branches are evaluated by libclang against the complete BLE core header tree
from the same immutable Git tag. This resolves included and function-like macros with the real C
preprocessor instead of approximating `#if` expressions in Rust. Running the compliance checker
therefore requires a clang driver and a loadable libclang installation (for example,
`libclang-dev` on Debian or Ubuntu).

## Usage

This crate works with controllers that implement `bt_hci::controller::Controller` and the
proprietary ST HCI specification. Standard commands and events come directly from the public
`bt_hci` re-export. Vendor commands are generated command types: construct one, then call
`SyncCmd::exec` for a Command Complete command or `AsyncCmd::exec` for a Command Status command.
The adapter executes both standard and vendor types through `ControllerCmdSync` and
`ControllerCmdAsync`. Decoded vendor events borrow variable-length fields directly from the
controller packet, so they remain compact and cannot outlive the read buffer.

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
