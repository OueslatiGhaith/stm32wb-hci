# STM32WB-HCI

forked from [bluetooth_hci](https://github.com/danielgallagher0/bluetooth-hci)

[![Build Status](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)](https://github.com/OueslatiGhaith/stm32wb-hci/actions/workflows/ci.yml/badge.svg)

This crate defines a pure Rust implementation of the [Bluetooth Host-Controller Interface](https://github.com/STMicroelectronics/STM32CubeWB/) for the STM32WB family of microcontrollers, and for the BLE stack library of STM32WBA. It declares
ST's vendor (ACI) and system (SHCI) commands and events.

## Selecting the wireless binary

Every command and event is checked at compile time against a catalog extracted from ST's
[STM32CubeWB](https://github.com/STMicroelectronics/STM32CubeWB) sources, for the wireless binary
running on CPU2. Select it with exactly one release feature and one stack profile feature:

```toml
stm32wb-hci = { version = "0.20", default-features = false, features = ["fw_1_24_0", "stack-full-extended"] }
```

- `fw_1_15_0` to `fw_1_24_0`: the STM32CubeWB release of the binary.
- `stack-full-extended`, `stack-full`, `stack-light`, `stack-hci-layer-extended`,
  `stack-hci-layer`, `stack-hci-adv-scan`: its BLE stack profile.

The binaries combining BLE with Thread, Zigbee or the 802.15.4 MAC (`stm32wb5x_BLE_Thread_*`,
`stm32wb5x_BLE_Zigbee_*`, `stm32wb5x_BLE_Mac_802_15_4_fw.bin`) carry the full BLE stack, as ST's
release notes state: select `stack-full` for them. This crate covers only their BLE side and the
system commands common to every binary, not the Thread, Zigbee or 802.15.4 commands.

### STM32WBA

The BLE stack library of STM32WBA is selected the same way, from ST's
[STM32CubeWBA](https://github.com/STMicroelectronics/STM32CubeWBA) sources:

```toml
stm32wb-hci = { version = "0.20", default-features = false, features = ["wba_1_10_0", "stack-wba-full"] }
```

- `wba_1_0_0` to `wba_1_10_0`: the STM32CubeWBA release of the library.
- `stack-wba-full`, `stack-wba-basic-plus`, `stack-wba-basic-features`,
  `stack-wba-peripheral-only`, `stack-wba-link-layer-only`,
  `stack-wba-link-layer-only-basic`: its configuration. Basic Plus and Peripheral Only exist from
  1.4.0, and Link Layer Only Basic before 1.8.0; selecting a configuration the release lacks fails
  to compile.

The same vendor and Bluetooth Core commands and events are declared for STM32WBA, checked against
its catalog. It has no system channel, so `stm32wb_hci::shci` is STM32WB only.

## Usage

A transport implements bt-hci's `Controller`, `ControllerCmdSync` and `ControllerCmdAsync`. Bounding
the command impls with `stm32wb_hci::catalog::Supported` restricts them to the commands the selected
binary implements, so any other fails to compile rather than being answered with Unknown HCI Command.
[`examples/embassy`](examples/embassy) has such a transport over the STM32WB55's IPCC.

Core commands are bt-hci's, or those of `stm32wb_hci::standard`; vendor commands are those of
`stm32wb_hci::aci`. `AciEvent::from_vendor` decodes a bt-hci vendor event. Events must be read, as
below, for commands to complete:

```rust
use bt_hci::cmd::controller_baseband::Reset;
use bt_hci::controller::{Controller, ControllerCmdSync};
use stm32wb_hci::aci::{
    flags::Role, gap::GapInit, gatt::GattInit, hal::HalWriteConfigData, values::Privacy,
};

join(
    async {
        loop {
            let mut buf = ();
            let pkt = ble.read(&mut buf).await;
            defmt::info!("pkt: {}", pkt);
        }
    },
    async {
        let response = ble.exec(&Reset::new()).await;
        defmt::info!("reset: {}", response.is_ok());

        // Offset 0x00 holds the public address, least significant byte first.
        let public_address = [0xE7, 0xCA, 0x10, 0x01, 0x00, 0xE1];
        let command = HalWriteConfigData::try_new(0x00, &public_address).unwrap();
        let response = ble.exec(&command).await;
        defmt::info!("write config data: {}", response.is_ok());

        let response = ble.exec(&GattInit::new()).await;
        defmt::info!("init gatt: {}", response.is_ok());

        // Peripheral role, without privacy, with an 8-byte device name.
        let response = ble
            .exec(&GapInit::new(Role::PERIPHERAL, Privacy::Disabled, 8))
            .await;
        defmt::info!("init gap: {}", response.is_ok());
    },
)
.await;
```

System commands, such as `stm32wb_hci::shci::BleInit`, go through the CPU2 system channel as
`stm32wb_hci::wire::SystemCommand`s.
