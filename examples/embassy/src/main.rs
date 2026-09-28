#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]

use crate::transport::ControllerAdapter;
use bt_hci::cmd::controller_baseband::Reset;
use bt_hci::controller::{Controller, ControllerCmdSync};
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::{
    bind_interrupts,
    ipcc::{Config as IpccConfig, ReceiveInterruptHandler, TransmitInterruptHandler},
    rcc::WPAN_DEFAULT,
};
use stm32wb_hci::{
    aci::{gap::GapInit, gatt::GattInit, hal::HalWriteConfigData, values::Privacy},
    shci::{BleInit, BleInitParams},
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
    let (mut sys, ble, mm) = transport::init(p.IPCC, Irqs, IpccConfig::default());

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

    if sys.command(&ble_init()).await.is_err() {
        error!("BLE stack init failed");
        return;
    }

    let ble = ControllerAdapter::new(ble);

    join(
        async {
            loop {
                let mut buf = ();
                let pkt = ble.read(&mut buf).await;

                defmt::info!("pkt: {}", pkt);
            }
        },
        async {
            defmt::info!("hci: reset");
            // From this point `ble` executes the bt-hci commands the selected target's
            // wireless binary implements: Core ones from bt-hci, vendor ones from `aci`.
            let response = ble.exec(&Reset::new()).await;
            defmt::info!("{}", response.is_ok());

            defmt::info!("hci: write config data");
            // Offset 0x00 holds the public address, least significant byte first.
            let public_address = [0xE7, 0xCA, 0x10, 0x01, 0x00, 0xE1];
            let response = match HalWriteConfigData::try_new(0x00, &public_address) {
                Ok(command) => ble.exec(&command).await.is_ok(),
                Err(_) => false,
            };
            defmt::info!("{}", response);

            defmt::info!("hci: init gatt");
            let response = ble.exec(&GattInit::new()).await;
            defmt::info!("{}", response.is_ok());

            defmt::info!("hci: init gap");
            // Peripheral role, without privacy, with an 8-byte device name.
            let response = ble.exec(&GapInit::new(0x01, Privacy::Disabled, 8)).await;
            defmt::info!("{}", response.is_ok());

            info!("BLE HCI ready");
        },
    )
    .await;
}

/// The BLE stack configuration, in the layout of the selected release.
fn ble_init() -> BleInit {
    BleInit::from(BleInitParams {
        p_ble_buffer_address: 0,
        ble_buffer_size: 0,
        num_attr_record: 68,
        num_attr_serv: 4,
        attr_value_arr_size: 1344,
        num_of_links: 2,
        extended_packet_length_enable: 1,
        pr_write_list_size: 0x3A,
        mblock_count: 0x79,
        att_mtu: 156,
        peripheral_sca: 500,
        central_sca: 0,
        ls_source: 1,
        max_conn_event_length: 0xFFFF_FFFF,
        hs_startup_time: 0x148,
        viterbi_enable: 1,
        options: 0,
        hw_version: 0,
        max_coc_initiator_nbr: 32,
        min_tx_power: -40,
        max_tx_power: 6,
        rx_model_config: 0,
        max_adv_set_nbr: 2,
        max_adv_data_len: 1650,
        tx_path_compens: 0,
        rx_path_compens: 0,
        ble_core_version: 11,
        options_extension: 0,
        max_add_eatt_bearers: 4,
        // No extra data buffer.
        extra_data_buffer: 0,
        extra_data_buffer_size: 0,
    })
}
