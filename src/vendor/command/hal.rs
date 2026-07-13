//! Vendor-specific HCI commands and types needed for those commands.

extern crate byteorder;

use bt_hci::{cmd::SyncCmd, controller::ControllerCmdSync};
use byteorder::{ByteOrder, LittleEndian};

use crate::{
    BadStatusError, Status,
    vendor::{
        command::BoundedBytes,
        event::command::{
            ClientStatus, HalAnchorPeriod, HalConfigData, HalConfigParameter, HalLinkStatus,
            LinkState,
        },
    },
};

impl TryFrom<BoundedBytes<16>> for HalConfigData {
    type Error = Error;

    fn try_from(value: BoundedBytes<16>) -> Result<Self, Self::Error> {
        let bytes = value.as_slice();
        let value = match bytes.len() {
            1 => HalConfigParameter::Byte(bytes[0]),
            2 => HalConfigParameter::Diversifier(LittleEndian::read_u16(bytes)),
            6 => {
                let mut address = [0; 6];
                address.copy_from_slice(bytes);
                HalConfigParameter::PublicAddress(crate::BdAddr(address))
            }
            16 => {
                let mut key = [0; 16];
                key.copy_from_slice(bytes);
                HalConfigParameter::EncryptionKey(crate::host::EncryptionKey(key))
            }
            other => {
                return Err(crate::event::Error::Vendor(
                    crate::vendor::event::VendorError::BadConfigParameterLength(other),
                )
                .into());
            }
        };
        Ok(Self { value })
    }
}

impl crate::vendor::command::HciDecodeField<16> for [u16; 8] {
    fn from_hci_field(bytes: &[u8; 16]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(core::array::from_fn(|index| {
            LittleEndian::read_u16(&bytes[index * 2..index * 2 + 2])
        }))
    }
}

impl crate::vendor::command::HciDecodeField<44> for [u16; 22] {
    fn from_hci_field(bytes: &[u8; 44]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(core::array::from_fn(|index| {
            LittleEndian::read_u16(&bytes[index * 2..index * 2 + 2])
        }))
    }
}

/// Vendor-specific HCI commands.
pub trait HalCommands {
    /// This command is intended to retrieve the firmware revision number.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a
    /// [command complete](crate::event::command::CommandComplete) event.
    ///
    /// The STM32WB generated API calls this a build number and returns it as
    /// a 16-bit value. It remains widened to `u64` here for source
    /// compatibility with the pre-feature-gating API.
    async fn get_firmware_revision(&self) -> Result<u64, Error>;

    /// This command writes a value to a low level configure data structure. It is useful to setup
    /// directly some low level parameters for the system in the runtime.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn write_config_data(&self, config: &ConfigData) -> Result<(), Error>;

    /// This command requests the value in the low level configure data structure.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn read_config_data(&self, param: ConfigParameter) -> Result<HalConfigData, Error>;

    /// This command sets the TX power level of the BlueNRG-MS.
    ///
    /// When the system starts up or reboots, the default TX power level will be used, which is the
    /// maximum value of [6 dBm](PowerLevel::Plus6dBm). Once this command is given, the output power
    /// will be changed instantly, regardless if there is Bluetooth communication going on or
    /// not. For example, for debugging purpose, the BlueNRG-MS can be set to advertise all the
    /// time. And use this command to observe the signal strength changing.
    ///
    /// The system will keep the last received TX power level from the command, i.e. the 2nd
    /// command overwrites the previous TX power level. The new TX power level remains until
    /// another Set TX Power command, or the system reboots.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn set_tx_power_level(&self, level: PowerLevel) -> Result<(), Error>;

    /// Retrieve the number of packets sent in the last TX direct test.
    ///
    /// During the Direct Test mode, in the TX tests, the number of packets sent in the test is not
    /// returned when executing the Direct Test End command. This command implements this feature.
    ///
    /// If the Direct TX test is started, a 16-bit counter will be used to count how many packets
    /// have been transmitted. After the Direct Test End, this command can be used to check how many
    /// packets were sent during the Direct TX test.
    ///
    /// The counter starts from 0 and counts upwards. As would be the case if 16-bits are all used,
    /// the counter wraps back and starts from 0 again. The counter is not cleared until the next
    /// Direct TX test starts.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn get_tx_test_packet_count(&self) -> Result<HalTxTestPacketCount, Error>;

    /// This command starts a carrier frequency, i.e. a tone, on a specific channel.
    ///
    /// The frequency sine wave at the specific channel may be used for debugging purpose only. The
    /// channel ID is a parameter from 0 to 39 for the 40 BLE channels, e.g. 0 for 2.402 GHz, 1 for
    /// 2.404 GHz etc.
    ///
    /// This command should not be used when normal Bluetooth activities are ongoing.
    /// The tone should be stopped by [`stop_tone`](HalCommands::stop_tone) command.
    ///
    /// # Errors
    ///
    /// - [InvalidChannel](Error::InvalidChannel) if the channel is greater than 39.
    /// - Underlying communication errors
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn start_tone(&self, channel: u8, freq_offset: u8) -> Result<(), Error>;

    /// Stops the previously started by the [`start_tone`](HalCommands::start_tone) command.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn stop_tone(&self) -> Result<(), Error>;

    /// This command is intended to return the Link Layer Status and Connection Handles.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn get_link_status(&self) -> Result<HalLinkStatus, Error>;

    /// This command sets the bitmask associated to
    /// [End of Radio Activity](crate::vendor::event::VendorEvent::HalEndOfRadioActivity) event.
    ///
    /// Only the radio activities enabled in the mask will be reported to the application by the
    /// [End of Radio Activity](crate::vendor::event::VendorEvent::HalEndOfRadioActivity) event.
    async fn set_radio_activity_mask(&self, mask: RadioActivityFlags) -> Result<(), Error>;

    /// This command is intended to retrieve information about the current Anchor Interval and
    /// allocable timing slots.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command complete](crate::event::command::CommandComplete) event.
    async fn get_anchor_period(&self) -> Result<HalAnchorPeriod, Error>;

    /// This command is used to enable/disable the generation of HAL events.
    ///
    /// If the bit in the [HAL Event Mask](HalEventFlags) is set to one, then the event associated with
    /// that will be enabled.
    async fn set_event_mask(&self, mask: HalEventFlags) -> Result<(), Error>;

    /// This command is used to retreive Tx, Rx, and total buffer count allocated for ACL packets.
    async fn get_pm_debug_info(&self) -> Result<HalPmDebugInfo, Error>;

    /// This command is used to disable/enable the Peripheral latencyy feature during a connection.
    ///
    /// Note that, by default, the Peripheral latency is enabled at connection time.
    async fn set_peripheral_latency(&self, enabled: bool) -> Result<(), Error>;

    /// This command returns the value of the RSSI.
    async fn read_rssi(&self) -> Result<u8, Error>;

    /// This command reads a register value from the RF module
    async fn read_radio_reg(&self, address: u8) -> Result<u8, Error>;

    /// This command writes a register value to the RF module.
    async fn write_radio_reg(&self, address: u8, value: u8) -> Result<(), Error>;

    /// This command returns the three raw RSSI bytes reported by the
    /// STM32WB firmware.
    async fn read_raw_rssi(&self) -> Result<[u8; 3], Error>;

    /// This command does set up the RF to listen to a specific RF Channel.
    ///
    /// `rf_channel`: BLE Channel Id, from 0x00 to 0x27 meaning `(2.402 + 0.002 * 0xXX) GHz`.
    /// The device will continously emit 0s, meaning that the tone will be at the channel center
    /// frequency minus the maximum frequency deviation (250 KHz).
    async fn rx_start(&self, rf_channel: u8) -> Result<(), Error>;

    /// This command stops a previous [HAL Rx Start](HalCommands::rx_start) command
    async fn rx_stop(&self) -> Result<(), Error>;

    /// This command is equivalent to [HCI Reset](crate::host::HostHci::reset) but ensures
    /// the sleep mode is entered immediately after its completion.
    async fn stack_reset(&self) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Returns the status of BLE links (up to 20 links plus 2 ISO streams).
    async fn get_link_status_v2(&self) -> Result<HalLinkStatusV2, Error>;

    #[cfg(after_fw_0_17_1)]
    /// Configure ACI_HAL_SYNC_EVENT.
    async fn set_sync_event_config(
        &self,
        group_id: u8,
        enable_sync: bool,
        enable_cb_trigger: bool,
        trigger_source: SyncTriggerSource,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Start continuous transmit test mode.
    async fn continuous_tx_start(
        &self,
        rf_channel: u8,
        phy: ContinuousTxPhy,
        pattern: ContinuousTxPattern,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Encrypt or decrypt data using the Encrypted Advertising Data scheme.
    async fn ead_encrypt_decrypt(&self, params: &EadParams) -> Result<HalEadResult, Error>;
}

vendor_cmd! {
    HalGetFirmwareRevision(HAL_GET_FIRMWARE_REVISION) {
        Params = ();
        Completion = CommandComplete;
        Return = HalFirmwareRevision {
            revision: u16 => 2,
        };
    }
}

vendor_cmd! {
    HalWriteConfigData(HAL_WRITE_CONFIG_DATA) {
        Params<'a> = {
            offset: u8 => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 46,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalReadConfigData(HAL_READ_CONFIG_DATA) {
        Params = {
            param: ConfigParameter => 1,
        };
        Completion = CommandComplete;
        Return = HalReadConfigDataReturn {
            value: BoundedBytes<16> => {
                kind: trailing_bytes,
                min_len: 1,
                max_len: 16,
            },
        };
    }
}

vendor_cmd! {
    HalSetTxPowerLevel(HAL_SET_TX_POWER_LEVEL) {
        Params = {
            high_power_mode: bool => 1,
            power_level: PowerLevel => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetTxTestPacketCount(HAL_TX_TEST_PACKET_COUNT) {
        Params = ();
        Completion = CommandComplete;
        Return = HalTxTestPacketCount {
            packet_count: u32 => 4,
        };
    }
}

vendor_cmd! {
    HalStartTone(HAL_START_TONE) {
        Params = {
            channel: u8 => 1,
            freq_offset: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalStopTone(HAL_STOP_TONE) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetLinkStatus(HAL_GET_LINK_STATUS) {
        Params = ();
        Completion = CommandComplete;
        Return = HalLinkStatusRaw {
            link_status: [u8; 8] => 8,
            link_connection_handles: [u16; 8] => 16,
        };
    }
}

vendor_cmd! {
    HalSetRadioActivityMask(HAL_SET_RADIO_ACTIVITY_MASK) {
        Params = {
            mask: RadioActivityFlags => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetAnchorPeriod(HAL_GET_ANCHOR_PERIOD) {
        Params = ();
        Completion = CommandComplete;
        Return = HalAnchorPeriodRaw {
            anchor_interval: u32 => 4,
            max_slot: u32 => 4,
        };
    }
}

vendor_cmd! {
    HalSetEventMask(HAL_SET_EVENT_MASK) {
        Params = {
            mask: HalEventFlags => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetPmDebugInfo(HAL_GET_PM_DEBUG_INFO) {
        Params = ();
        Completion = CommandComplete;
        Return = HalPmDebugInfo {
            tx: u8 => 1,
            rx: u8 => 1,
            mblocks: u8 => 1,
        };
    }
}

vendor_cmd! {
    HalSetPeripheralLatency(HAL_SET_PERIPHERAL_LATENCY) {
        Params = {
            enabled: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalReadRssi(HAL_READ_RSSI) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRssi {
            value: u8 => 1,
        };
    }
}

vendor_cmd! {
    HalReadRadioReg(HAL_READ_RADIO_REG) {
        Params = {
            address: u8 => 1,
        };
        Completion = CommandComplete;
        Return = HalRadioRegisterValue {
            value: u8 => 1,
        };
    }
}

vendor_cmd! {
    HalWriteRadioReg(HAL_WRITE_RADIO_REG) {
        Params = {
            address: u8 => 1,
            value: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalReadRawRssi(HAL_READ_RAW_RSSI) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRawRssi {
            value: [u8; 3] => 3,
        };
    }
}

vendor_cmd! {
    HalRxStart(HAL_RX_START) {
        Params = {
            rf_channel: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalRxStop(HAL_RX_STOP) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalStackReset(HAL_STACK_RESET) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalGetLinkStatusV2(HAL_GET_LINK_STATUS_V2) {
        Params = ();
        Completion = CommandComplete;
        Return = HalLinkStatusV2Raw {
            link_status: [u8; 22] => 22,
            link_connection_handles: [u16; 22] => 44,
        };
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalSetSyncEventConfig(HAL_SET_SYNC_EVENT_CONFIG) {
        Params = {
            group_id: u8 => 1,
            enable_sync: bool => 1,
            enable_cb_trigger: bool => 1,
            trigger_source: SyncTriggerSource => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalContinuousTxStart(HAL_CONTINUOUS_TX_START) {
        Params = {
            rf_channel: u8 => 1,
            phy: ContinuousTxPhy => 1,
            pattern: ContinuousTxPattern => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalEadEncryptDecrypt(HAL_EAD_ENCRYPT_DECRYPT) {
        Params<'a> = {
            mode: EadMode => 1,
            key: &'a [u8; 16] => 16,
            iv: &'a [u8; 8] => 8,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 228,
            },
        };
        Completion = CommandComplete;
        Return = HalEadEncryptDecryptReturn {
            data: BoundedBytes<237> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 237,
            },
        };
    }
}

cfg_command_bounds! {
    HalFirmwareCommands,
    after_fw_0_17_1,
    ControllerCmdSync<HalGetLinkStatusV2>
        + ControllerCmdSync<HalSetSyncEventConfig>
        + ControllerCmdSync<HalContinuousTxStart>
        + for<'t> ControllerCmdSync<HalEadEncryptDecrypt<'t>>
}

impl<T> HalCommands for T
where
    T: ControllerCmdSync<HalGetFirmwareRevision>
        + for<'t> ControllerCmdSync<HalWriteConfigData<'t>>
        + ControllerCmdSync<HalReadConfigData>
        + ControllerCmdSync<HalSetTxPowerLevel>
        + ControllerCmdSync<HalGetTxTestPacketCount>
        + ControllerCmdSync<HalStartTone>
        + ControllerCmdSync<HalStopTone>
        + ControllerCmdSync<HalGetLinkStatus>
        + ControllerCmdSync<HalSetRadioActivityMask>
        + ControllerCmdSync<HalGetAnchorPeriod>
        + ControllerCmdSync<HalSetEventMask>
        + ControllerCmdSync<HalGetPmDebugInfo>
        + ControllerCmdSync<HalSetPeripheralLatency>
        + ControllerCmdSync<HalReadRssi>
        + ControllerCmdSync<HalReadRawRssi>
        + ControllerCmdSync<HalReadRadioReg>
        + ControllerCmdSync<HalWriteRadioReg>
        + ControllerCmdSync<HalRxStart>
        + ControllerCmdSync<HalRxStop>
        + ControllerCmdSync<HalStackReset>
        + HalFirmwareCommands,
{
    async fn get_firmware_revision(&self) -> Result<u64, Error> {
        let revision = HalGetFirmwareRevision::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(u64::from(revision.revision))
    }

    async fn write_config_data(&self, config: &ConfigData) -> Result<(), Error> {
        let value = &config.value_buf[..usize::from(config.length)];
        HalWriteConfigData::try_new(config.offset, value)?
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn read_config_data(&self, param: ConfigParameter) -> Result<HalConfigData, Error> {
        let value = HalReadConfigData::new(param)
            .exec(self)
            .await
            .map_err(Error::from)?
            .value;
        value.try_into()
    }

    async fn set_tx_power_level(&self, level: PowerLevel) -> Result<(), Error> {
        // High power mode is deprecated and ignored on STM32WB.
        HalSetTxPowerLevel::new(false, level)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_tx_test_packet_count(&self) -> Result<HalTxTestPacketCount, Error> {
        HalGetTxTestPacketCount::new()
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn start_tone(&self, channel: u8, freq_offset: u8) -> Result<(), Error> {
        const MAX_CHANNEL: u8 = 39;
        if channel > MAX_CHANNEL {
            return Err(Error::InvalidChannel(channel));
        }

        HalStartTone::new(channel, freq_offset)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn stop_tone(&self) -> Result<(), Error> {
        HalStopTone::new().exec(self).await.map_err(|e| e.into())
    }

    async fn get_link_status(&self) -> Result<HalLinkStatus, Error> {
        let raw = HalGetLinkStatus::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        let mut clients = [ClientStatus {
            state: LinkState::Idle,
            conn_handle: crate::ConnectionHandle(0),
        }; 8];
        for (index, client) in clients.iter_mut().enumerate() {
            client.state = raw.link_status[index]
                .try_into()
                .map_err(crate::event::Error::Vendor)?;
            client.conn_handle = crate::ConnectionHandle(raw.link_connection_handles[index]);
        }
        Ok(HalLinkStatus { clients })
    }

    async fn get_anchor_period(&self) -> Result<HalAnchorPeriod, Error> {
        let raw = HalGetAnchorPeriod::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(HalAnchorPeriod {
            anchor_interval: core::time::Duration::from_micros(
                625 * u64::from(raw.anchor_interval),
            ),
            max_slot: core::time::Duration::from_micros(625 * u64::from(raw.max_slot)),
        })
    }

    async fn set_radio_activity_mask(&self, mask: RadioActivityFlags) -> Result<(), Error> {
        HalSetRadioActivityMask::new(mask)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn set_event_mask(&self, mask: HalEventFlags) -> Result<(), Error> {
        HalSetEventMask::new(mask)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_pm_debug_info(&self) -> Result<HalPmDebugInfo, Error> {
        HalGetPmDebugInfo::new()
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn set_peripheral_latency(&self, enabled: bool) -> Result<(), Error> {
        HalSetPeripheralLatency::new(enabled)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn read_rssi(&self) -> Result<u8, Error> {
        Ok(HalReadRssi::new()
            .exec(self)
            .await
            .map_err(Error::from)?
            .value)
    }

    async fn read_radio_reg(&self, address: u8) -> Result<u8, Error> {
        Ok(HalReadRadioReg::new(address)
            .exec(self)
            .await
            .map_err(Error::from)?
            .value)
    }

    async fn write_radio_reg(&self, address: u8, value: u8) -> Result<(), Error> {
        HalWriteRadioReg::new(address, value)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn read_raw_rssi(&self) -> Result<[u8; 3], Error> {
        let rssi = HalReadRawRssi::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(rssi.value)
    }

    async fn rx_start(&self, rf_channel: u8) -> Result<(), Error> {
        HalRxStart::new(rf_channel)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn rx_stop(&self) -> Result<(), Error> {
        HalRxStop::new().exec(self).await.map_err(|e| e.into())
    }

    async fn stack_reset(&self) -> Result<(), Error> {
        HalStackReset::new().exec(self).await.map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn get_link_status_v2(&self) -> Result<HalLinkStatusV2, Error> {
        let raw = HalGetLinkStatusV2::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(HalLinkStatusV2 {
            link_status: raw.link_status,
            link_connection_handles: raw.link_connection_handles,
        })
    }

    #[cfg(after_fw_0_17_1)]
    async fn set_sync_event_config(
        &self,
        group_id: u8,
        enable_sync: bool,
        enable_cb_trigger: bool,
        trigger_source: SyncTriggerSource,
    ) -> Result<(), Error> {
        HalSetSyncEventConfig::new(group_id, enable_sync, enable_cb_trigger, trigger_source)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn continuous_tx_start(
        &self,
        rf_channel: u8,
        phy: ContinuousTxPhy,
        pattern: ContinuousTxPattern,
    ) -> Result<(), Error> {
        HalContinuousTxStart::new(rf_channel, phy, pattern)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn ead_encrypt_decrypt(&self, params: &EadParams) -> Result<HalEadResult, Error> {
        let data = params.data.get(..params.data_len).ok_or_else(|| {
            crate::vendor::command::HciLengthError::new(params.data_len, 0, params.data.len())
        })?;
        let result = HalEadEncryptDecrypt::try_new(params.mode, &params.key, &params.iv, data)?
            .exec(self)
            .await
            .map_err(Error::from)?;
        let out_len = result.data.as_slice().len();
        let mut data = [0u8; 248];
        data[..out_len].copy_from_slice(result.data.as_slice());
        Ok(HalEadResult {
            data,
            data_len: out_len,
        })
    }
}

/// Potential errors from parameter validation.
///
/// Before some commands are sent to the controller, the parameters are validated. This type
/// enumerates the potential validation errors. Must be specialized on the types of communication
/// errors.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// For the [Start Tone](HalCommands::start_tone) command, the channel was greater than the maximum
    /// allowed channel (39). The invalid channel is returned.
    InvalidChannel(u8),

    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// A variable-length parameter exceeds the command's wire bounds.
    InvalidParameterLength(crate::vendor::command::HciLengthError),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl From<crate::vendor::command::HciLengthError> for Error {
    fn from(error: crate::vendor::command::HciLengthError) -> Self {
        Self::InvalidParameterLength(error)
    }
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
    }
}

/// Low-level configuration parameters for the controller.
pub struct ConfigData {
    /// Offset of the element in the configuration data structure which has to be written.
    ///
    /// Values:
    ///- 0x00: CONFIG_DATA_PUBADDR_OFFSET;
    ///  Bluetooth public address; 6 bytes
    ///- 0x08: CONFIG_DATA_ER_OFFSET;
    ///  Encryption root key used to derive LTK (legacy) and CSRK; 16 bytes
    ///- 0x18: CONFIG_DATA_IR_OFFSET;
    ///  Identity root key used to derive DHK (legacy) and IRK; 16 bytes
    ///- 0x2E: CONFIG_DATA_RANDOM_ADDRESS_OFFSET;
    ///  Static Random Address; 6 bytes
    ///- 0x34: CONFIG_DATA_GAP_ADD_REC_NBR_OFFSET;
    ///  GAP service additional record number; 1 byte
    ///- 0x35: CONFIG_DATA_SC_KEY_TYPE_OFFSET;
    ///  Secure Connection key type (0: "normal", 1: "debug"); 1 byte
    ///- 0xB0: CONFIG_DATA_SMP_MODE_OFFSET;
    ///  SMP mode (0: "normal", 1: "bypass", 2: "no blacklist"); 1 byte
    ///- 0xC0: CONFIG_DATA_LL_SCAN_CHAN_MAP_OFFSET (only for STM32WB);
    ///  LL scan channel map (same format as Primary_Adv_Channel_Map); 1
    ///  byte
    ///- 0xC1: CONFIG_DATA_LL_BG_SCAN_MODE_OFFSET (only for STM32WB);
    ///  LL background scan mode (0: "BG scan disabled", 1: "BG scan
    ///  enabled"); 1 byte
    offset: u8,
    /// Length of the value to be written
    length: u8,
    /// Data to be written
    value_buf: [u8; ConfigData::MAX_LENGTH],
}

impl ConfigData {
    /// Maximum length needed to serialize the data.
    pub const MAX_LENGTH: usize = 0x2E;

    /// Serializes the data into the given buffer.
    ///
    /// Returns the number of valid bytes in the buffer.
    ///
    /// # Panics
    ///
    /// The buffer must be large enough to support the serialized data (at least
    /// [`MAX_LENGTH`](ConfigData::MAX_LENGTH) bytes).
    pub fn copy_into_slice(&self, bytes: &mut [u8]) -> usize {
        bytes[0] = self.offset;
        bytes[1] = self.length;

        let len = self.length as usize;
        bytes[2..2 + len].copy_from_slice(&self.value_buf[..len]);

        2 + len
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn public_address(addr: crate::BdAddr) -> ConfigDataDiversifierBuilder {
        let mut data = Self {
            offset: 0,
            length: 6,
            value_buf: [0; Self::MAX_LENGTH],
        };

        data.value_buf[0..6].copy_from_slice(&addr.0);

        ConfigDataDiversifierBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn random_address(addr: crate::BdAddr) -> ConfigDataDiversifierBuilder {
        let mut data = Self {
            offset: 0x2E,
            length: 6,
            value_buf: [0; Self::MAX_LENGTH],
        };

        data.value_buf[0..6].copy_from_slice(&addr.0);

        ConfigDataDiversifierBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn diversifier(d: u16) -> ConfigDataEncryptionRootBuilder {
        let mut data = Self {
            offset: 6,
            length: 2,
            value_buf: [0; Self::MAX_LENGTH],
        };
        LittleEndian::write_u16(&mut data.value_buf[0..2], d);

        ConfigDataEncryptionRootBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn encryption_root(key: &crate::host::EncryptionKey) -> ConfigDataIdentityRootBuilder {
        let mut data = Self {
            offset: 8,
            length: 16,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0..16].copy_from_slice(&key.0);

        ConfigDataIdentityRootBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn identity_root(key: &crate::host::EncryptionKey) -> ConfigDataLinkLayerOnlyBuilder {
        let mut data = Self {
            offset: 24,
            length: 16,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0..16].copy_from_slice(&key.0);
        ConfigDataLinkLayerOnlyBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn link_layer_only(ll_only: bool) -> ConfigDataRoleBuilder {
        let mut data = Self {
            offset: 40,
            length: 1,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0] = ll_only as u8;
        ConfigDataRoleBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalCommands::write_config_data).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn role(role: Role) -> ConfigDataCompleteBuilder {
        let mut data = Self {
            offset: 41,
            length: 1,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0] = role as u8;
        ConfigDataCompleteBuilder { data }
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataDiversifierBuilder {
    data: ConfigData,
}

impl ConfigDataDiversifierBuilder {
    /// Specify the diversifier and continue building.
    pub fn diversifier(mut self, d: u16) -> ConfigDataEncryptionRootBuilder {
        let len = self.data.length as usize;
        LittleEndian::write_u16(&mut self.data.value_buf[len..2 + len], d);
        self.data.length += 2;

        ConfigDataEncryptionRootBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes only the public address.
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataEncryptionRootBuilder {
    data: ConfigData,
}

impl ConfigDataEncryptionRootBuilder {
    /// Specify the encryption root and continue building.
    pub fn encryption_root(
        mut self,
        key: &crate::host::EncryptionKey,
    ) -> ConfigDataIdentityRootBuilder {
        let len = self.data.length as usize;
        self.data.value_buf[len..16 + len].copy_from_slice(&key.0);
        self.data.length += 16;

        ConfigDataIdentityRootBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the diversifier, and may include fields before it,
    /// but does not include any fields after it (including the encryption root).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataIdentityRootBuilder {
    data: ConfigData,
}

impl ConfigDataIdentityRootBuilder {
    /// Specify the identity root and continue building.
    pub fn identity_root(
        mut self,
        key: &crate::host::EncryptionKey,
    ) -> ConfigDataLinkLayerOnlyBuilder {
        let len = self.data.length as usize;
        self.data.value_buf[len..16 + len].copy_from_slice(&key.0);
        self.data.length += 16;

        ConfigDataLinkLayerOnlyBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the encryption root, and may include fields before
    /// it, but does not include any fields after it (including the identity root).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataLinkLayerOnlyBuilder {
    data: ConfigData,
}

impl ConfigDataLinkLayerOnlyBuilder {
    /// Specify whether to use the link layer only and continue building.
    pub fn link_layer_only(mut self, ll_only: bool) -> ConfigDataRoleBuilder {
        self.data.value_buf[self.data.length as usize] = ll_only as u8;
        self.data.length += 1;
        ConfigDataRoleBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the identity root, and may include fields before
    /// it, but does not include any fields after it (including the link layer only flag).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataRoleBuilder {
    data: ConfigData,
}

impl ConfigDataRoleBuilder {
    /// Specify the device role and continue building.
    pub fn role(mut self, role: Role) -> ConfigDataCompleteBuilder {
        self.data.value_buf[self.data.length as usize] = role as u8;
        self.data.length += 1;
        ConfigDataCompleteBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the link layer only flag, and may include fields
    /// before it, but does not include any fields after it (including the role).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataCompleteBuilder {
    data: ConfigData,
}

impl ConfigDataCompleteBuilder {
    /// Build the [ConfigData] as-is. It includes the role field, and may include fields before it.
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Roles that the server can adopt.
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Role {
    /// Peripheral and primary device.
    /// - Only one connection.
    /// - 6 KB of RAM retention.
    Peripheral6Kb = 1,

    /// Peripheral and primary device.
    /// - Only one connection.
    /// - 12 KB of RAM retention.
    Peripheral12Kb = 2,

    /// Primary device and peripheral
    /// - Up to 8 connections
    /// - 12 KB of RAM retention
    Primary12Kb = 3,

    /// Primary device and peripheral.
    /// - Simultaneous advertising and scanning
    /// - Up to 4 connections
    /// - This mode is available starting from BlueNRG-MS FW stack version 7.1.b
    SimultaneousAdvertisingScanning = 4,
}

/// Configuration parameters that are readable by the
/// [`read_config_data`](HalCommands::read_config_data) command.
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigParameter {
    /// Bluetooth public address.
    PublicAddress = 0,

    /// Bluetooth random address.
    RandomAddress = 0x2E,

    /// Diversifier used to derive CSRK (connection signature resolving key).
    Diversifier = 6,

    /// Encryption root key used to derive the LTK (long-term key) and CSRK (connection signature
    /// resolving key).
    EncryptionRoot = 8,

    /// Identity root key used to derive the LTK (long-term key) and CSRK (connection signature
    /// resolving key).
    IdentityRoot = 24,

    /// Switch on/off Link Layer only mode.
    LinkLayerOnly = 40,

    /// BlueNRG-MS roles and mode configuration.
    Role = 41,
}

impl crate::vendor::command::HciEncodeField<1> for ConfigParameter {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8])
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8]).await
    }
}

/// Transmitter power levels available for the system.
///
/// STM32WB5x uses single byte parameter for PA level.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerLevel {
    /// -40 dBm.
    Minus40dBm = 0x00,

    /// -20.85 dBm.
    Minus20_85dBm = 0x01,

    /// -19.75 dBm.
    Minus19_75dBm = 0x02,

    /// -18.85 dBm.
    Minus18_85dBm = 0x03,

    /// 17.6 dBm.
    Minus17_6dBm = 0x04,

    /// -16.5 dBm.
    Minus16_5dBm = 0x05,

    /// -15.25 dBm.
    Minus15_25dBm = 0x06,

    /// -14.1 dBm.
    Minus14_1dBm = 0x07,

    /// -13.15 dBm.
    Minus13_15dBm = 0x08,

    /// -12.05 dBm.
    Minus12_05dBm = 0x09,

    /// -10.9 dBm.
    Minus10_9dBm = 0x0A,

    /// -9.9 dBm.
    Minus9_9dBm = 0x0B,

    /// -8.85 dBm.
    Minus8_85dBm = 0x0C,

    /// -7.8 dBm.
    Minus7_8dBm = 0x0D,

    /// -6.9 dBm.
    Minus6_9dBm = 0x0E,

    /// -5.9 dBm.
    Minus5_9dBm = 0x0F,

    /// -4.95 dBm.
    Minus4_95dBm = 0x10,

    /// -4 dBm.
    Minus4dBm = 0x11,

    /// -3.15 dBm.
    Minus3_15dBm = 0x12,

    /// -2.45 dBm.
    Minus2_45dBm = 0x13,

    /// -1.8 dBm.
    Minus1_8dBm = 0x14,

    /// -1.3 dBm.
    Minus1_3dBm = 0x15,

    /// -0.85 dBm.
    Minus0_85dBm = 0x16,

    /// -0.5 dBm.
    Minus0_5dBm = 0x17,

    /// -0.15 dBm.
    Minus0_15dBm = 0x18,

    /// 0 dBm.
    ZerodBm = 0x19,

    /// 1 dBm.
    Plus1dBm = 0x1A,

    /// 2 dBm.
    Plus2dBm = 0x1B,

    /// 3 dBm.
    Plus3dBm = 0x1C,

    /// 4 dBm.
    Plus4dBm = 0x1D,

    /// 5 dBm.
    Plus5dBm = 0x1E,

    /// 6 dBm.
    Plus6dBm = 0x1F,
}

impl crate::vendor::command::HciEncodeField<1> for PowerLevel {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8])
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8]).await
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct RadioActivityFlags: u16 {
        /// Idle
        const IDLE = 0x0001;
        /// Advertising
        const ADVERTISING = 0x0002;
        /// Peripheral connection
        const PERIPHERAL_CONN = 0x0004;
        /// Scanning
        const SCANNING = 0x0008;
        /// Central connection
        const CENTRAL_CONN = 0x0020;
        /// Tx test mode
        const TX_TEST = 0x0040;
        /// Rx test mode
        const RX_TEST = 0x0080;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    pub struct RadioActivityFlags: u16 {
        /// Idle
        const IDLE = 0x0001;
        /// Advertising
        const ADVERTISING = 0x0002;
        /// Peripheral connection
        const PERIPHERAL_CONN = 0x0004;
        /// Scanning
        const SCANNING = 0x0008;
        /// Central connection
        const CENTRAL_CONN = 0x0020;
        /// Tx test mode
        const TX_TEST = 0x0040;
        /// Rx test mode
        const RX_TEST = 0x0080;
    }
}

impl crate::vendor::command::HciEncodeField<2> for RadioActivityFlags {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.bits().write_hci_field(writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.bits().write_hci_field_async(writer).await
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct HalEventFlags: u32 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    pub struct HalEventFlags: u32 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}

impl crate::vendor::command::HciEncodeField<4> for HalEventFlags {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.bits().write_hci_field(writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.bits().write_hci_field_async(writer).await
    }
}

#[cfg(after_fw_0_17_1)]
/// Return value for [get_link_status_v2](HalCommands::get_link_status_v2).
pub struct HalLinkStatusV2 {
    /// Link statuses for up to 20 links + 2 ISO streams.
    pub link_status: [u8; 22],
    /// Connection handles for each link (0 if not connected).
    pub link_connection_handles: [u16; 22],
}

/// Trigger source for [set_sync_event_config](HalCommands::set_sync_event_config).
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SyncTriggerSource {
    Cig = 0x00,
    Big = 0x01,
}

/// PHY for [continuous_tx_start](HalCommands::continuous_tx_start).
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContinuousTxPhy {
    Le1M = 0x01,
    Le2M = 0x02,
}

/// Data pattern for [continuous_tx_start](HalCommands::continuous_tx_start).
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContinuousTxPattern {
    Prbs9 = 0x00,
    Alternating11110000 = 0x01,
    Alternating10101010 = 0x02,
    Prbs15 = 0x03,
    AllOnes = 0x04,
    AllZeros = 0x05,
    Alternating00001111 = 0x06,
    Alternating0101 = 0x07,
}

/// Mode for [ead_encrypt_decrypt](HalCommands::ead_encrypt_decrypt).
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EadMode {
    Encrypt = 0x00,
    Decrypt = 0x01,
}

macro_rules! impl_u8_hci_field {
    ($type:ty) => {
        impl crate::vendor::command::HciEncodeField<1> for $type {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&[*self as u8])
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&[*self as u8]).await
            }
        }
    };
}

impl_u8_hci_field!(SyncTriggerSource);
impl_u8_hci_field!(ContinuousTxPhy);
impl_u8_hci_field!(ContinuousTxPattern);
impl_u8_hci_field!(EadMode);

#[cfg(after_fw_0_17_1)]
/// Parameters for [ead_encrypt_decrypt](HalCommands::ead_encrypt_decrypt).
pub struct EadParams {
    /// EAD operation mode.
    pub mode: EadMode,
    /// Session key (16 bytes, little-endian).
    pub key: [u8; 16],
    /// Initialization vector (8 bytes, little-endian).
    pub iv: [u8; 8],
    /// Input data (up to 248 bytes).
    pub data: [u8; 248],
    /// Length of valid data in `data`.
    pub data_len: usize,
}

#[cfg(after_fw_0_17_1)]
/// Return value for [ead_encrypt_decrypt](HalCommands::ead_encrypt_decrypt).
pub struct HalEadResult {
    /// Result data.
    pub data: [u8; 248],
    /// Length of valid data in `data`.
    pub data_len: usize,
}
