#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    ipcc::{Config as IpccConfig, ReceiveInterruptHandler, TransmitInterruptHandler},
    rcc::WPAN_DEFAULT,
};
use stm32wb_hci::{
    BdAddr, Controller, Opcode,
    host::{HostHci, uart::UartHci},
    vendor::command::{gap::GapCommands, gatt::GattCommands, hal::HalCommands},
};
use {defmt_rtt as _, panic_probe as _};

mod transport;

bind_interrupts!(struct Irqs {
    IPCC_C1_RX => ReceiveInterruptHandler;
    IPCC_C1_TX => TransmitInterruptHandler;
});

#[embassy_executor::task]
async fn release_event_buffers(mut mm: transport::MemoryManager<'static>) -> ! {
    mm.run_queue().await
}

#[embassy_executor::main(
    executor = "embassy_stm32::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc = WPAN_DEFAULT;
    let p = embassy_stm32::init(config);

    // This transport module is a stripped-down version of embassy-stm32-wpan.
    // It keeps only what this example needs: CPU2 startup, memory-buffer release,
    // SHCI BLE init, and raw BLE HCI command/event transport.
    let (mut sys, mut ble, mm) = transport::init(p.IPCC, Irqs, IpccConfig::default());

    // CPU2 must announce that wireless firmware is running before SHCI commands are valid.
    info!("wait CPU2 ready event");
    match sys.read_ready().await {
        Ok(transport::SysEventReady::WirelessFwRunning) => info!("wireless firmware running"),
        Ok(_) => {
            error!("CPU2 not running wireless firmware");
            return;
        }
        Err(()) => {
            error!("bad CPU2 ready event");
            return;
        }
    }

    // CPU2 owns event buffers until host returns them through the memory-manager channel.
    match release_event_buffers(mm) {
        Ok(task) => spawner.spawn(task),
        Err(_) => {
            error!("failed to spawn memory manager");
            return;
        }
    }

    if sys
        .ble_init(transport::BleInitParam::default())
        .await
        .is_err()
    {
        error!("BLE stack init failed");
        return;
    }

    // From this point `ble` implements `stm32wb_hci::Controller` below. All commands
    // after this line are normal stm32wb-hci host/vendor commands, not transport code.
    ble.reset().await;
    log_next_event(&mut ble).await;

    let public_address = BdAddr([0xE7, 0xCA, 0x10, 0x01, 0x00, 0xE1]);
    ble.write_config_data(
        &stm32wb_hci::vendor::command::hal::ConfigData::public_address(public_address).build(),
    )
    .await;
    log_next_event(&mut ble).await;

    ble.init_gatt().await;
    log_next_event(&mut ble).await;

    ble.init_gap(
        stm32wb_hci::vendor::command::gap::Role::PERIPHERAL,
        false,
        8,
    )
    .await;
    log_next_event(&mut ble).await;

    info!("BLE HCI ready");

    loop {
        log_next_event(&mut ble).await;
    }
}

async fn log_next_event(ble: &mut transport::Ble<'_>) {
    match ble.read().await {
        Ok(packet) => info!("HCI packet: {:?}", packet),
        Err(e) => error!("HCI read error: {:?}", e),
    }
}

impl<'a> Controller for transport::Ble<'a> {
    async fn controller_write(&mut self, opcode: Opcode, payload: &[u8]) {
        self.write_hci_command(opcode.0, payload).await;
    }

    async fn controller_read_into(&mut self, buf: &mut [u8]) {
        self.read_hci_event_into(buf).await;
    }
}
