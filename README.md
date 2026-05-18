# STM32WB-HCI

forked from [bluetooth_hci](https://github.com/danielgallagher0/bluetooth-hci)

[![Build Status](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)

This crate defines a pure Rust implementation of the [Bluetooth Host-Controller Interface](https://github.com/STMicroelectronics/STM32CubeWB/) for the STM32WB family of microcontrollers. It defines commands
and events from the specification, and vendor-specific commands and events.

## Version

This crate aims to match the [latest firmware binaries](https://github.com/STMicroelectronics/STM32CubeWB/tree/master/Projects/STM32WB_Copro_Wireless_Binaries/STM32WB5x) released by ST. The minor version number of this crate should indicate the appropriate firmware version to use, refer to this table in unclear:

| crate version   | firmware version |
| --------------- | ---------------- |
| 0.17.2 / 0.17.3 | 1.17.1           |
| 0.17.0          | 1.17.0           |
| 0.16.0          | 1.16.0           |
| older           | 1.15.0           |

## Usage

This crate works with any controller that implements `bt_hci::Controller` and the proprietary ST
HCI specification. Currently, this includes stm32wb and stm32wb microcontroller families. 

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
            // From this point `ble` implements `stm32wb_hci::Controller` below. All commands
            // after this line are normal stm32wb-hci host/vendor commands, not transport code.
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