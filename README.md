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
`before_fw_0_17_0`, `only_fw_0_17_0`, `after_fw_0_17_0`, and `since_fw_0_17_0`. `before` and
`after` are strict comparisons; `since` includes the named firmware. Do not use
`--all-features`, because firmware features are intentionally mutually exclusive. The shorter
`before_0_17_0`/`only_0_17_0`/`after_0_17_0`/`since_0_17_0` spellings are emitted as compatibility
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

`--deny` makes differences fail the command for CI; omit it to inspect the report, or pass
`--json` for machine-readable output. `--all-supported` discovers canonical
`fw_<major>_<minor>_<patch>` entries directly from the crate's `[features]` table, so adding a
firmware feature automatically puts it in the compliance and CI loops.

The checker reports the requested CubeWB tag, the tag object, and the resolved commit, making a
result reproducible even when inspecting a local CubeWB clone. Its exclusions are governed by the
checked-in [policy](tools/compliance/exclusions.policy). The policy requires a reason for every
exception and rejects malformed, overlapping, unsupported-version, or stale entries; an exception
must actively suppress a real coverage difference. Pass `--policy <path>` only when evaluating a
deliberate alternative policy.

The checker parses both the Rust crate and CubeWB's generated C sources as syntax trees, and
evaluates the selected firmware cfgs, so module-, trait-, impl-, method-, and branch-level gates
are all included in the active API inventory. It compares:

- vendor ACI command IDs and vendor-event IDs from CubeWB's generated `ble_*_aci.c` and
  `ble_events.c` files;
- standard HCI command opcodes, ordinary events, and LE Meta subevents from `ble_hci_le.c` and
  `ble_events.c` against the crate's public `bt_hci` re-export plus STM32WB command extensions;
- vendor command envelopes: empty versus non-empty requests, Command Complete versus Command
  Status completion, and exact response lengths for literal and fixed packed C layouts.

The four CubeWB responses whose capacity is configured by `BLE_EVT_MAX_PARAM_LEN` are reported as
`wire unavailable` rather than guessed: HAL Read Config Data, GAP Get Bonded Devices, GATT Read
Handle Value, and L2CAP CoC Connect Confirm. This keeps `--deny` meaningful while making the
remaining schema work explicit. All transport-only exceptions—including the coprocessor-ready
event `0x9200`—come exclusively from the checked-in policy; library defaults do not hide them.

## Usage

This crate works with controllers that implement `bt_hci::controller::Controller` and the
proprietary ST HCI specification. Command traits such as `stm32wb_hci::host::HostHci` and the
vendor command traits are implemented for adapters that can execute the relevant `bt-hci` command
types through `ControllerCmdSync` and `ControllerCmdAsync`.

The `read_packet` function may have to be polled for commands to complete. A channel or other
methods may be used to accomplish this so that `read_packet` is never in a state where it is not
polled.

```rust
    let ble = ControllerAdapter::new(ble);

    join(
        async {
            loop {
                let pkt = ble.read_packet().await;

                defmt::info!("pkt: {}", pkt);
            }
        },
        async {
            // From this point `ble` implements the bt-hci controller traits. All commands after
            // this line are normal stm32wb-hci host/vendor commands, not transport code.
            let response = ble.reset().await;
            defmt::info!("{}", response);

            let public_address = BdAddr([0xE7, 0xCA, 0x10, 0x01, 0x00, 0xE1]);
            let response = ble
                .write_config_data(
                    &stm32wb_hci::vendor::command::hal::ConfigData::public_address(public_address)
                        .build(),
                )
                .await;
            defmt::info!("{}", response);

            let response = ble.init_gatt().await;
            defmt::info!("{}", response);

            let response = ble
                .init_gap(
                    stm32wb_hci::vendor::command::gap::Role::PERIPHERAL,
                    false,
                    8,
                )
                .await;
            defmt::info!("{}", response);

            info!("BLE HCI ready");
        },
    )
    .await;
```
