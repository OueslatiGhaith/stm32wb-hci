//! Return parameters for vendor-specific commands.
//!
//! This module defines the parameters returned in the Command Complete event for vendor-specific
//! commands.  These commands are defined for the BlueNRG controller, but are not standard HCI
//! commands.
//!
//! ## Coverage notes
//!
//! This module intentionally focuses on vendor opcodes that generate
//! `Command Complete` return parameters.
//!
//! A significant subset of vendor commands are procedure-oriented and are handled through
//! `Command Status` plus follow-up vendor/LE events instead of rich command-complete payloads.
//! Those opcodes are expected to be absent from [`VendorReturnParameters`] decode match arms.
//!
//! Typical event-driven examples:
//! - GAP: discovery/connection/security procedure starters (`GAP_START_*`, `GAP_CREATE_CONNECTION`,
//!   `GAP_SEND_PAIRING_REQUEST`, `GAP_PERIPHERAL_SECURITY_REQUEST`, `GAP_TERMINATE`)
//! - GATT client procedures (`GATT_FIND_*`, `GATT_DISCOVER_*`, `GATT_READ_*`, `GATT_WRITE_*`,
//!   `GATT_PREPARE_WRITE_REQUEST`, `GATT_EXECUTE_WRITE_REQUEST`, `GATT_EXCHANGE_CONFIGURATION`)
//! - L2CAP connection parameter update request (`L2CAP_CONN_PARAM_UPDATE_REQ`)
//!
//! When adding support for a new vendor command, first verify whether the controller reports useful
//! data in `Command Complete` for that opcode; if not, handle completion via the corresponding event.

use byteorder::{ByteOrder, LittleEndian};
use core::convert::{TryFrom, TryInto};
use core::time::Duration;

use super::AttributeHandle;
/// Parameters returned by GAP commands with declarative payloads.
pub use crate::vendor::command::gap::{GapBondedDevices, GapInit};
/// Parameters returned by the GATT Read Handle Value command.
pub use crate::vendor::command::gatt::GattHandleValue;
/// Parameters returned by GATT server-definition commands.
pub use crate::vendor::command::gatt::{
    GattCharacteristic, GattCharacteristicDescriptor, GattService,
};
/// Parameters returned by declarative HAL commands.
pub use crate::vendor::command::hal::{HalFirmwareRevision, HalPmDebugInfo, HalTxTestPacketCount};
use crate::vendor::command::hal::{HalRawRssi, HalRssi};

/// Vendor-specific commands that may generate the
/// [Command Complete](crate::event::command::ReturnParameters::Vendor) event. If the commands have defined
/// return parameters, they are included in the enum.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorReturnParameters {
    /// Parameters returned by the
    /// [HAL Get Firmware Revision](crate::vendor::command::hal::HalGetFirmwareRevision) command.
    HalGetFirmwareRevision(HalFirmwareRevision),

    /// Status returned by the [HAL Write Config Data](crate::vendor::command::hal::HalWriteConfigData)
    /// command.
    HalWriteConfigData(crate::Status),

    /// Parameters returned by the [HAL Read Config Data](crate::vendor::command::hal::HalReadConfigData)
    /// command.
    HalReadConfigData(HalConfigData),

    /// Status returned by the [HAL Set Tx Power Level](crate::vendor::command::hal::HalSetTxPowerLevel)
    /// command.
    HalSetTxPowerLevel(crate::Status),

    /// Status returned by the
    /// HAL Device Standby command.
    #[cfg(after_fw_0_17_1)]
    HalDeviceStandby(crate::Status),

    /// Parameters returned by the
    /// [HAL Get Tx Test Packet Count](crate::vendor::command::hal::HalGetTxTestPacketCount) command.
    HalGetTxTestPacketCount(HalTxTestPacketCount),

    /// Status returned by the [HAL Start Tone](crate::vendor::command::hal::HalStartTone) command.
    HalStartTone(crate::Status),

    /// Status returned by the [HAL Stop Tone](crate::vendor::command::hal::HalStopTone) command.
    HalStopTone(crate::Status),

    /// Status returned by the [HAL Get Link Status](crate::vendor::command::hal::HalGetLinkStatus) command.
    HalGetLinkStatus(HalLinkStatus),

    /// Parameters returned by the [HAL Get Anchor Period](crate::vendor::command::hal::HalGetAnchorPeriod)
    /// command.
    HalGetAnchorPeriod(HalAnchorPeriod),

    /// Parameters returned by the [HAL Get PM Debug Info](crate::vendor::command::hal::HalGetPmDebugInfo)
    /// command.
    HalGetPmDebugInfo(HalPmDebugInfo),

    /// Status returned by the
    /// [HAL Set Radio Activity Mask](crate::vendor::command::hal::HalSetRadioActivityMask)
    /// command.
    HalSetRadioActivityMask(crate::Status),

    /// Status returned by the
    /// [HAL Set Event Mask](crate::vendor::command::hal::HalSetEventMask) command.
    HalSetEventMask(crate::Status),

    /// Status returned by the
    /// [HAL Set Peripheral Latency](crate::vendor::command::hal::HalSetPeripheralLatency)
    /// command.
    HalSetPeripheralLatency(crate::Status),

    /// Parameters returned by the [HAL Read RSSI](crate::vendor::command::hal::HalReadRssi)
    /// command.
    HalReadRssi(u8),

    /// Parameters returned by the [HAL Read Radio Register](crate::vendor::command::hal::HalReadRadioReg)
    /// command.
    HalReadRadioReg(u8),

    /// Status returned by the [HAL Write Radio Register](crate::vendor::command::hal::HalWriteRadioReg)
    /// command.
    HalWriteRadioReg(crate::Status),

    /// Parameters returned by the [HAL Read Raw RSSI](crate::vendor::command::hal::HalReadRawRssi)
    /// command.
    HalReadRawRssi([u8; 3]),

    /// Status returned by the [HAL RX Start](crate::vendor::command::hal::HalRxStart) command.
    HalRxStart(crate::Status),

    /// Status returned by the [HAL RX Stop](crate::vendor::command::hal::HalRxStop) command.
    HalRxStop(crate::Status),

    /// Status returned by the [HAL Stack Reset](crate::vendor::command::hal::HalStackReset) command.
    HalStackReset(crate::Status),

    /// Status returned by the
    /// [GAP Set Non-Discoverable](crate::vendor::command::gap::GapSetNonDiscoverable)
    /// command.
    GapSetNonDiscoverable(crate::Status),

    /// Status returned by the
    /// [GAP Set Discoverable](crate::vendor::command::gap::GapSetDiscoverable)
    /// command.
    GapSetDiscoverable(crate::Status),

    /// Status returned by the
    /// [GAP Set Direct Connectable](crate::vendor::command::gap::GapSetDirectConnectable) command.
    GapSetDirectConnectable(crate::Status),

    /// Status returned by the [GAP Set IO Capability](crate::vendor::command::gap::GapSetIoCapability)
    /// command.
    GapSetIoCapability(crate::Status),

    /// Status returned by the
    /// [GAP Set Authentication Requirement](crate::vendor::command::gap::GapSetAuthenticationRequirement) command.
    GapSetAuthenticationRequirement(crate::Status),

    /// Status returned by the
    /// [GAP Set Authorization Requirement](crate::vendor::command::gap::GapSetAuthorizationRequirement) command.
    GapSetAuthorizationRequirement(crate::Status),

    /// Status returned by the
    /// [GAP Pass Key Response](crate::vendor::command::gap::GapPassKeyResponse)
    /// command.
    GapPassKeyResponse(crate::Status),

    /// Status returned by the
    /// [GAP Authorization Response](crate::vendor::command::gap::GapAuthorizationResponse) command.
    GapAuthorizationResponse(crate::Status),

    /// Parameters returned by the [GAP Init](crate::vendor::command::gap::CmdGapInit) command.
    GapInit(GapInit),

    /// Parameters returned by the
    /// [GAP Set Non-Connectable](crate::vendor::command::gap::GapSetNonConnectable) command.
    GapSetNonConnectable(crate::Status),

    /// Parameters returned by the
    /// [GAP Set Undirected Connectable](crate::vendor::command::gap::GapSetUnidirectedConnectable) command.
    GapSetUndirectedConnectable(crate::Status),

    /// Parameters returned by the
    /// [GAP Update Advertising Data](crate::vendor::command::gap::GapUpdateAdvertisingData) command.
    GapUpdateAdvertisingData(crate::Status),

    /// Parameters returned by the
    /// [GAP Delete AD Type](crate::vendor::command::gap::GapDeleteAdType)
    /// command.
    GapDeleteAdType(crate::Status),

    /// Parameters returned by the
    /// [GAP Get Security Level](crate::vendor::command::gap::GapGetSecurityLevel) command.
    GapGetSecurityLevel(GapSecurityLevel),

    /// Parameters returned by the
    /// [GAP Set Event Mask](crate::vendor::command::gap::GapSetEventMask)
    /// command.
    GapSetEventMask(crate::Status),

    /// Parameters returned by the
    /// [GAP Configure White List](crate::vendor::command::gap::GapConfigureWhitelist) command.
    GapConfigureWhiteList(crate::Status),

    /// Parameters returned by the
    /// [GAP Clear Security Database](crate::vendor::command::gap::GapClearSecurityDatabase) command.
    GapClearSecurityDatabase(crate::Status),

    /// Parameters returned by the
    /// [GAP Allow Rebond](crate::vendor::command::gap::GapAllowRebond) command.
    GapAllowRebond(crate::Status),

    /// Parameters returned by the
    /// [GAP Terminate Procedure](crate::vendor::command::gap::GapTerminateProcedure) command.
    GapTerminateProcedure(crate::Status),

    /// Parameters returned by the
    /// [GAP Resolve Private Address](crate::vendor::command::gap::CmdGapResolvePrivateAddress) command.
    GapResolvePrivateAddress(GapResolvePrivateAddress),

    /// Parameters returned by the
    /// [GAP Get Bonded Devices](crate::vendor::command::gap::GapGetBondedDevices) command.
    GapGetBondedDevices(GapBondedDevices),

    /// Parameters returned by the
    /// [GAP Set Broadcast Mode](crate::vendor::command::gap::GapSetBroadcastMode) command.
    GapSetBroadcastMode(crate::Status),

    /// Parameters returned by the
    /// [GAP Start Observation Procedure](crate::vendor::command::gap::GapStartObservationProcedure) command.
    GapStartObservationProcedure(crate::Status),

    /// Parameters returned by the
    /// [GAP Is Device Bonded](crate::vendor::command::gap::GapIsDeviceBonded)
    /// command.
    GapIsDeviceBonded(crate::Status),

    /// Parameters returned by the
    /// [GAP Pairing Request Reply](crate::vendor::command::gap::GapPairingRequestReply)
    /// command.
    #[cfg(after_fw_0_17_1)]
    GapPairingRequestReply(crate::Status),

    /// Parameters returned by the
    /// [GAP Get OOB Data](crate::vendor::command::gap::GapGetOobData) command.
    GapGetOobData((crate::Status, [u8; 26])),

    /// Parameters returned by the
    /// [GAP Passkey Input](crate::vendor::command::gap::GapPasskeyInput) command.
    GapPasskeyInput(crate::Status),

    /// Parameters returned by the
    /// [GAP Set OOB Data](crate::vendor::command::gap::GapSetOobData) command.
    GapSetOobData(crate::Status),

    /// Parameters returned by the
    /// [GAP Add Devices To Resolving List](crate::vendor::command::gap::GapAddDevicesToResolvingList)
    /// command.
    GapAddDevicesToResolvingList(crate::Status),

    /// Parameters returned by the
    /// [GAP Remove Bonded Device](crate::vendor::command::gap::GapRemoveBondedDevice)
    /// command.
    GapRemoveBondedDevice(crate::Status),

    /// Parameters returned by the
    /// [GAP Add Devices To List](crate::vendor::command::gap::GapAddDevicesToList) command.
    GapAddDevicesToList(crate::Status),

    /// Parameters returned by the
    /// [GAP Additional Beacon Start](crate::vendor::command::gap::GapAdditionalBeaconStart)
    /// command.
    GapAdditionalBeaconStart(crate::Status),

    /// Parameters returned by the
    /// [GAP Additional Beacon Stop](crate::vendor::command::gap::GapAdditionalBeaconStop)
    /// command.
    GapAdditionalBeaconStop(crate::Status),

    /// Parameters returned by the
    /// [GAP Additional Beacon Set Data](crate::vendor::command::gap::GapAdditionalBeaconSetData)
    /// command.
    GapAdditionalBeaconSetData(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Set Configuration](crate::vendor::command::gap::GapAdvSetConfig)
    /// command.
    GapAdvSetConfiguration(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Set Enable](crate::vendor::command::gap::GapAdvSetEnable) command.
    GapAdvSetEnable(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Set Advertising Data](crate::vendor::command::gap::GapAdvSetAdvertisingData)
    /// command.
    GapAdvSetAdvertisingData(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Set Scan Response Data](crate::vendor::command::gap::GapAdvSetScanResponseData)
    /// command.
    GapAdvSetScanResponseData(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Remove Set](crate::vendor::command::gap::GapAdvRemoveSet) command.
    GapAdvRemoveSet(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Clear Sets](crate::vendor::command::gap::GapAdvClearSets) command.
    GapAdvClearSets(crate::Status),

    /// Parameters returned by the
    /// [GAP Adv Set Random Address](crate::vendor::command::gap::GapAdvSetRandomAddress)
    /// command.
    GapAdvSetRandomAddress(crate::Status),

    /// Parameters returned by the
    /// [GATT Init](crate::vendor::command::gatt::GattInit) command.
    GattInit(crate::Status),

    /// Parameters returned by the
    /// [GATT Add Service](crate::vendor::command::gatt::GattAddService) command.
    GattAddService(GattService),

    /// Parameters returned by the
    /// [GATT Include Service](crate::vendor::command::gatt::GattIncludeService)
    /// command.
    GattIncludeService(GattService),

    /// Parameters returned by the
    /// [GATT Add Characteristic](crate::vendor::command::gatt::GattAddCharacteristic) command.
    GattAddCharacteristic(GattCharacteristic),

    /// Parameters returned by the
    /// [GATT Add Characteristic Descriptor](crate::vendor::command::gatt::GattAddCharacteristicDescriptor) command.
    GattAddCharacteristicDescriptor(GattCharacteristicDescriptor),

    /// Parameters returned by the
    /// [GATT Update Characteristic Value](crate::vendor::command::gatt::GattUpdateCharacteristicValue) command.
    GattUpdateCharacteristicValue(crate::Status),

    /// Parameters returned by the
    /// [GATT Delete Characteristic](crate::vendor::command::gatt::GattDeleteCharacterisitic) command.
    GattDeleteCharacteristic(crate::Status),

    /// Parameters returned by the
    /// [GATT Delete Service](crate::vendor::command::gatt::GattDeleteService)
    /// command.
    GattDeleteService(crate::Status),

    /// Parameters returned by the
    /// [GATT Delete Included Service](crate::vendor::command::gatt::GattDeleteIncludedService) command.
    GattDeleteIncludedService(crate::Status),

    /// Parameters returned by the [GATT Set Event Mask](crate::vendor::command::gatt::GattSetEventMask)
    /// command.
    GattSetEventMask(crate::Status),

    /// Parameters returned by the
    /// [GATT Write Without Response](crate::vendor::command::gatt::GattWriteWithoutResponse) command.
    GattWriteWithoutResponse(crate::Status),

    /// Parameters returned by the
    /// [GATT Signed Write Without Response](crate::vendor::command::gatt::GattSignedWriteWithoutResponse) command.
    GattSignedWriteWithoutResponse(crate::Status),

    /// Parameters returned by the
    /// [GATT Confirm Indication](crate::vendor::command::gatt::GattConfirmIndication) command.
    GattConfirmIndication(crate::Status),

    /// Parameters returned by the [GATT Write Response](crate::vendor::command::gatt::GattWriteResponse)
    /// command.
    GattWriteResponse(crate::Status),

    /// Parameters returned by the [GATT Allow Read](crate::vendor::command::gatt::GattAllowRead) command.
    GattAllowRead(crate::Status),

    /// Parameters returned by the
    /// [GATT Set Security Permission](crate::vendor::command::gatt::GattSetSecurityPermission) command.
    GattSetSecurityPermission(crate::Status),

    /// Parameters returned by the
    /// [GATT Set Descriptor Value](crate::vendor::command::gatt::GattSetDescriptorValue) command.
    GattSetDescriptorValue(crate::Status),

    /// Parameters returned by the
    /// GATT Read Handle Value command.
    GattReadHandleValue(GattHandleValue),

    /// Parameters returned by the
    /// [GATT Read Handle Value](crate::vendor::command::gatt::GattReadHandleValueOffset) command.
    #[cfg(after_fw_0_17_1)]
    GattReadHandleValueOffset(GattHandleValue),

    /// Parameters returned by the
    /// [GATT Update Long Characteristic Value](crate::vendor::command::gatt::GattUpdateLongCharacteristicValue) command.
    GattUpdateLongCharacteristicValue(crate::Status),

    /// Parameters returned by the
    /// [GATT Deny Read](crate::vendor::command::gatt::GattDenyRead) command.
    GattDenyRead(crate::Status),

    /// Parameters returned by the
    /// [GATT Set Access Permission](crate::vendor::command::gatt::GattSetAccessPermission)
    /// command.
    GattSetAccessPermission(crate::Status),

    /// Parameters returned by the
    /// [GATT Store DB](crate::vendor::command::gatt::GattStoreDatabase)
    GattStoreDb(crate::Status),

    /// Parameters returned by the
    /// [GATT Send Multiple Notification](crate::vendor::command::gatt::GattSendMultipleNotification)
    /// command.
    GattSendMultipleNotification(crate::Status),

    /// Parameters returned by the
    /// [GATT Read Multiple Variable Characteristic Value](crate::vendor::command::gatt::GattReadMultipleVarCharValue)
    /// command.
    GattReadMultipleVarCharValue(crate::Status),

    /// Status returned by the
    /// [L2CAP Connection Parameter Update Response](crate::vendor::command::l2cap::L2ConnectionParameterUpdateResponse) command.
    L2CapConnectionParameterUpdateResponse(crate::Status),

    /// Status returned by the
    /// [L2CAP COC Connect](crate::vendor::command::l2cap::L2CocConnect) command.
    L2CapCocConnect(crate::Status),

    /// Parameters returned by the
    /// [L2CAP COC Connect Confirm](crate::vendor::command::l2cap::L2CocConnectConfirm)
    /// command.
    L2CapCocConnectConfirm(L2CapCocConnectConfirmResponse),

    /// Status returned by the
    /// [L2CAP COC Reconfig](crate::vendor::command::l2cap::L2CocReconfig) command.
    L2CapCocReconfig(crate::Status),

    /// Status returned by the
    /// [L2CAP COC Reconfig Confirm](crate::vendor::command::l2cap::L2CocReconfigConfirm)
    /// command.
    L2CapCocReconfigConfirm(crate::Status),

    /// Status returned by the
    /// [L2CAP COC Disconnect](crate::vendor::command::l2cap::L2CocDisconnect) command.
    L2CapCocDisconnect(crate::Status),

    /// Status returned by the
    /// [L2CAP COC Flow Control](crate::vendor::command::l2cap::L2CocFlowControl)
    /// command.
    L2CapCocFlowControl(crate::Status),

    /// Status returned by the
    /// [L2CAP COC TX Data](crate::vendor::command::l2cap::L2CocTxData) command.
    L2CapCocTxData(crate::Status),
}

impl VendorReturnParameters {
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, crate::event::Error> {
        check_len_at_least(bytes, 3)?;

        match crate::Opcode(LittleEndian::read_u16(&bytes[1..])) {
            crate::vendor::opcode::HAL_GET_FIRMWARE_REVISION => {
                Ok(VendorReturnParameters::HalGetFirmwareRevision(
                    to_hal_firmware_revision(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::HAL_WRITE_CONFIG_DATA => Ok(
                VendorReturnParameters::HalWriteConfigData(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_READ_CONFIG_DATA => Ok(
                VendorReturnParameters::HalReadConfigData(to_hal_config_data(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_SET_TX_POWER_LEVEL => Ok(
                VendorReturnParameters::HalSetTxPowerLevel(to_status(&bytes[3..])?),
            ),
            #[cfg(after_fw_0_17_1)]
            crate::vendor::opcode::HAL_DEVICE_STANDBY => Ok(
                VendorReturnParameters::HalDeviceStandby(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_TX_TEST_PACKET_COUNT => {
                Ok(VendorReturnParameters::HalGetTxTestPacketCount(
                    to_hal_tx_test_packet_count(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::HAL_START_TONE => Ok(VendorReturnParameters::HalStartTone(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::HAL_STOP_TONE => {
                Ok(VendorReturnParameters::HalStopTone(to_status(&bytes[3..])?))
            }
            crate::vendor::opcode::HAL_GET_LINK_STATUS => Ok(
                VendorReturnParameters::HalGetLinkStatus(to_hal_link_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_GET_ANCHOR_PERIOD => Ok(
                VendorReturnParameters::HalGetAnchorPeriod(to_hal_anchor_period(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_GET_PM_DEBUG_INFO => Ok(
                VendorReturnParameters::HalGetPmDebugInfo(to_hal_pm_debug_info(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_SET_RADIO_ACTIVITY_MASK => Ok(
                VendorReturnParameters::HalSetRadioActivityMask(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_SET_EVENT_MASK => Ok(
                VendorReturnParameters::HalSetEventMask(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_SET_PERIPHERAL_LATENCY => Ok(
                VendorReturnParameters::HalSetPeripheralLatency(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_READ_RSSI => Ok(VendorReturnParameters::HalReadRssi(
                decode_status_prefixed::<HalRssi>(&bytes[3..], 1)?.value,
            )),
            crate::vendor::opcode::HAL_READ_RADIO_REG => {
                Ok(VendorReturnParameters::HalReadRadioReg({
                    require_len!(&bytes[3..], 1);
                    bytes[3]
                }))
            }
            crate::vendor::opcode::HAL_WRITE_RADIO_REG => Ok(
                VendorReturnParameters::HalWriteRadioReg(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::HAL_READ_RAW_RSSI => Ok(VendorReturnParameters::HalReadRawRssi(
                decode_status_prefixed::<HalRawRssi>(&bytes[3..], 3)?.value,
            )),
            crate::vendor::opcode::HAL_RX_START => {
                Ok(VendorReturnParameters::HalRxStart(to_status(&bytes[3..])?))
            }
            crate::vendor::opcode::HAL_RX_STOP => {
                Ok(VendorReturnParameters::HalRxStop(to_status(&bytes[3..])?))
            }
            crate::vendor::opcode::HAL_STACK_RESET => Ok(VendorReturnParameters::HalStackReset(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::GAP_SET_NONDISCOVERABLE => Ok(
                VendorReturnParameters::GapSetNonDiscoverable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_DISCOVERABLE => Ok(
                VendorReturnParameters::GapSetDiscoverable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_DIRECT_CONNECTABLE => Ok(
                VendorReturnParameters::GapSetDirectConnectable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_IO_CAPABILITY => Ok(
                VendorReturnParameters::GapSetIoCapability(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_AUTHENTICATION_REQUIREMENT => Ok(
                VendorReturnParameters::GapSetAuthenticationRequirement(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_AUTHORIZATION_REQUIREMENT => Ok(
                VendorReturnParameters::GapSetAuthorizationRequirement(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_PASS_KEY_RESPONSE => Ok(
                VendorReturnParameters::GapPassKeyResponse(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_AUTHORIZATION_RESPONSE => Ok(
                VendorReturnParameters::GapAuthorizationResponse(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_INIT => {
                Ok(VendorReturnParameters::GapInit(to_gap_init(&bytes[3..])?))
            }
            crate::vendor::opcode::GAP_SET_NONCONNECTABLE => Ok(
                VendorReturnParameters::GapSetNonConnectable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_UNDIRECTED_CONNECTABLE => Ok(
                VendorReturnParameters::GapSetUndirectedConnectable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_UPDATE_ADVERTISING_DATA => Ok(
                VendorReturnParameters::GapUpdateAdvertisingData(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_DELETE_AD_TYPE => Ok(
                VendorReturnParameters::GapDeleteAdType(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_GET_SECURITY_LEVEL => Ok(
                VendorReturnParameters::GapGetSecurityLevel(to_gap_security_level(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_EVENT_MASK => Ok(
                VendorReturnParameters::GapSetEventMask(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_CONFIGURE_WHITE_LIST => Ok(
                VendorReturnParameters::GapConfigureWhiteList(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_CLEAR_SECURITY_DATABASE => Ok(
                VendorReturnParameters::GapClearSecurityDatabase(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ALLOW_REBOND => Ok(VendorReturnParameters::GapAllowRebond(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::GAP_TERMINATE_PROCEDURE => Ok(
                VendorReturnParameters::GapTerminateProcedure(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_RESOLVE_PRIVATE_ADDRESS => {
                Ok(VendorReturnParameters::GapResolvePrivateAddress(
                    to_gap_resolve_private_address(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::GAP_GET_BONDED_DEVICES => Ok(
                VendorReturnParameters::GapGetBondedDevices(to_gap_bonded_devices(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_BROADCAST_MODE => Ok(
                VendorReturnParameters::GapSetBroadcastMode(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_START_OBSERVATION_PROCEDURE => Ok(
                VendorReturnParameters::GapStartObservationProcedure(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_IS_DEVICE_BONDED => Ok(
                VendorReturnParameters::GapIsDeviceBonded(to_status(&bytes[3..])?),
            ),
            #[cfg(after_fw_0_17_1)]
            crate::vendor::opcode::GAP_PAIRING_REQUEST_REPLY => Ok(
                VendorReturnParameters::GapPairingRequestReply(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_GET_OOB_DATA => Ok(VendorReturnParameters::GapGetOobData({
                require_len!(bytes, 26 - 1 - 3);

                (to_status(&bytes[3..4])?, bytes[4..].try_into().unwrap())
            })),
            crate::vendor::opcode::GAP_PASSKEY_INPUT => Ok(
                VendorReturnParameters::GapPasskeyInput(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_SET_OOB_DATA => Ok(VendorReturnParameters::GapSetOobData(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::GAP_ADD_DEVICES_TO_RESOLVING_LIST => Ok(
                VendorReturnParameters::GapAddDevicesToResolvingList(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_REMOVE_BONDED_DEVICE => Ok(
                VendorReturnParameters::GapRemoveBondedDevice(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADD_DEVICES_TO_LIST => Ok(
                VendorReturnParameters::GapAddDevicesToList(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADDITIONAL_BEACON_START => Ok(
                VendorReturnParameters::GapAdditionalBeaconStart(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADDITIONAL_BEACON_STOP => Ok(
                VendorReturnParameters::GapAdditionalBeaconStop(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADDITIONAL_BEACON_SET_DATA => Ok(
                VendorReturnParameters::GapAdditionalBeaconSetData(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_SET_CONFIGURATION => Ok(
                VendorReturnParameters::GapAdvSetConfiguration(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_SET_ENABLE => Ok(
                VendorReturnParameters::GapAdvSetEnable(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_SET_ADV_DATA => Ok(
                VendorReturnParameters::GapAdvSetAdvertisingData(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_SET_SCAN_RESPONSE_DATA => Ok(
                VendorReturnParameters::GapAdvSetScanResponseData(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_REMOVE_SET => Ok(
                VendorReturnParameters::GapAdvRemoveSet(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_CLEAR_SETS => Ok(
                VendorReturnParameters::GapAdvClearSets(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GAP_ADV_SET_RANDOM_ADDRESS => Ok(
                VendorReturnParameters::GapAdvSetRandomAddress(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_INIT => {
                Ok(VendorReturnParameters::GattInit(to_status(&bytes[3..])?))
            }
            crate::vendor::opcode::GATT_ADD_SERVICE => Ok(VendorReturnParameters::GattAddService(
                to_gatt_service(&bytes[3..])?,
            )),
            crate::vendor::opcode::GATT_INCLUDE_SERVICE => Ok(
                VendorReturnParameters::GattIncludeService(to_gatt_service(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_ADD_CHARACTERISTIC => Ok(
                VendorReturnParameters::GattAddCharacteristic(to_gatt_characteristic(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_ADD_CHARACTERISTIC_DESCRIPTOR => {
                Ok(VendorReturnParameters::GattAddCharacteristicDescriptor(
                    to_gatt_characteristic_descriptor(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::GATT_UPDATE_CHARACTERISTIC_VALUE => Ok(
                VendorReturnParameters::GattUpdateCharacteristicValue(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_DELETE_CHARACTERISTIC => Ok(
                VendorReturnParameters::GattDeleteCharacteristic(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_DELETE_SERVICE => Ok(
                VendorReturnParameters::GattDeleteService(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_DELETE_INCLUDED_SERVICE => Ok(
                VendorReturnParameters::GattDeleteIncludedService(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_SET_EVENT_MASK => Ok(
                VendorReturnParameters::GattSetEventMask(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_WRITE_WITHOUT_RESPONSE => Ok(
                VendorReturnParameters::GattWriteWithoutResponse(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_SIGNED_WRITE_WITHOUT_RESPONSE => Ok(
                VendorReturnParameters::GattSignedWriteWithoutResponse(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_CONFIRM_INDICATION => Ok(
                VendorReturnParameters::GattConfirmIndication(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_WRITE_RESPONSE => Ok(
                VendorReturnParameters::GattWriteResponse(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_ALLOW_READ => Ok(VendorReturnParameters::GattAllowRead(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::GATT_SET_SECURITY_PERMISSION => Ok(
                VendorReturnParameters::GattSetSecurityPermission(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_SET_DESCRIPTOR_VALUE => Ok(
                VendorReturnParameters::GattSetDescriptorValue(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_READ_HANDLE_VALUE => Ok(
                VendorReturnParameters::GattReadHandleValue(to_gatt_handle_value(&bytes[3..])?),
            ),
            #[cfg(after_fw_0_17_1)]
            crate::vendor::opcode::GATT_READ_HANDLE_VALUE_OFFSET => {
                Ok(VendorReturnParameters::GattReadHandleValueOffset(
                    to_gatt_handle_value(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::GATT_UPDATE_LONG_CHARACTERISTIC_VALUE => Ok(
                VendorReturnParameters::GattUpdateLongCharacteristicValue(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_DENY_READ => Ok(VendorReturnParameters::GattDenyRead(
                to_status(&bytes[3..])?,
            )),
            crate::vendor::opcode::GATT_SET_ACCESS_PERMISSION => Ok(
                VendorReturnParameters::GattSetAccessPermission(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_STORE_DB => {
                Ok(VendorReturnParameters::GattStoreDb(to_status(&bytes[3..])?))
            }
            crate::vendor::opcode::GATT_SEND_MULT_NOTIFICATION => Ok(
                VendorReturnParameters::GattSendMultipleNotification(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::GATT_READ_MULTIPLE_VAR_CHAR_VALUE => Ok(
                VendorReturnParameters::GattReadMultipleVarCharValue(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_CONN_PARAM_UPDATE_RESP => Ok(
                VendorReturnParameters::L2CapConnectionParameterUpdateResponse(to_status(
                    &bytes[3..],
                )?),
            ),
            crate::vendor::opcode::L2CAP_COC_CONNECT => Ok(
                VendorReturnParameters::L2CapCocConnect(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_COC_CONNECT_CONFIRM => {
                Ok(VendorReturnParameters::L2CapCocConnectConfirm(
                    to_l2cap_coc_connect_confirm_response(&bytes[3..])?,
                ))
            }
            crate::vendor::opcode::L2CAP_COC_RECONFIG => Ok(
                VendorReturnParameters::L2CapCocReconfig(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_COC_RECONFIG_CONFIRM => Ok(
                VendorReturnParameters::L2CapCocReconfigConfirm(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_COC_DISCONNECT => Ok(
                VendorReturnParameters::L2CapCocDisconnect(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_COC_FLOW_CONTROL => Ok(
                VendorReturnParameters::L2CapCocFlowControl(to_status(&bytes[3..])?),
            ),
            crate::vendor::opcode::L2CAP_COC_TX_DATA => Ok(VendorReturnParameters::L2CapCocTxData(
                to_status(&bytes[3..])?,
            )),
            other => Err(crate::event::Error::UnknownOpcode(other)),
        }
    }
}

fn check_len_at_least(buffer: &[u8], len: usize) -> Result<(), crate::event::Error> {
    if buffer.len() < len {
        Err(crate::event::Error::BadLength(buffer.len(), len))
    } else {
        Ok(())
    }
}

fn to_status(bytes: &[u8]) -> Result<crate::Status, crate::event::Error> {
    require_len_at_least!(bytes, 1);
    bytes[0].try_into().map_err(crate::event::rewrap_bad_status)
}

fn decode_status_prefixed<T>(bytes: &[u8], payload_len: usize) -> Result<T, crate::event::Error>
where
    for<'de> T: bt_hci::FromHciBytes<'de>,
{
    let expected_len = payload_len + 1;
    if bytes.len() != expected_len {
        return Err(crate::event::Error::BadLength(bytes.len(), expected_len));
    }
    <T as bt_hci::FromHciBytes>::from_hci_bytes_complete(&bytes[1..])
        .map_err(|_| crate::event::Error::BadLength(bytes.len(), expected_len))
}

fn to_hal_firmware_revision(bytes: &[u8]) -> Result<HalFirmwareRevision, crate::event::Error> {
    decode_status_prefixed(bytes, 2)
}

impl TryFrom<&[u8]> for HalFirmwareRevision {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_firmware_revision(bytes)
    }
}

/// Parameters returned by the [HAL Read Config Data](crate::vendor::command::hal::HalReadConfigData)
/// command.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HalConfigData {
    /// Requested value.
    ///
    /// The value is requested by offset, and distinguished upon return by length only. This means
    /// that this event cannot distinguish between the 16-byte encryption keys
    /// ([EncryptionRoot](crate::vendor::command::hal::ConfigParameter::EncryptionRoot) and
    /// [IdentityRoot](crate::vendor::command::hal::ConfigParameter::IdentityRoot)) or between the single-byte values
    /// ([LinkLayerOnly](crate::vendor::command::hal::ConfigParameter::LinkLayerOnly) or
    /// [Role](crate::vendor::command::hal::ConfigParameter::Role)).
    pub value: HalConfigParameter,
}

/// Potential values that can be fetched by
/// [HAL Read Config Data](crate::vendor::command::hal::HalReadConfigData).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HalConfigParameter {
    /// Bluetooth public address. Corresponds to
    /// [PublicAddress](crate::vendor::command::hal::ConfigParameter::PublicAddress).
    PublicAddress(crate::BdAddr),

    /// Bluetooth random address. Corresponds to
    /// [RandomAddress](crate::vendor::command::hal::ConfigParameter::RandomAddress).
    RandomAddress(crate::BdAddr),

    /// Diversifier used to derive CSRK (connection signature resolving key).  Corresponds to
    /// [Diversifier](crate::vendor::command::hal::ConfigParameter::Diversifier).
    Diversifier(u16),

    /// A requested encryption key. Corresponds to either
    /// [EncryptionRoot](crate::vendor::command::hal::ConfigParameter::EncryptionRoot) or
    /// [IdentityRoot](crate::vendor::command::hal::ConfigParameter::IdentityRoot).
    EncryptionKey(crate::host::EncryptionKey),

    /// A single-byte value. Corresponds to either
    /// [LinkLayerOnly](crate::vendor::command::hal::ConfigParameter::LinkLayerOnly) or
    /// [Role](crate::vendor::command::hal::ConfigParameter::Role).
    Byte(u8),
}

fn to_hal_config_data(bytes: &[u8]) -> Result<HalConfigData, crate::event::Error> {
    require_len_at_least!(bytes, 2);
    Ok(HalConfigData {
        value: to_hal_config_parameter(&bytes[1..])?,
    })
}

impl TryFrom<&[u8]> for HalConfigData {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_config_data(bytes)
    }
}

fn to_hal_config_parameter(bytes: &[u8]) -> Result<HalConfigParameter, crate::event::Error> {
    match bytes.len() {
        6 => {
            let mut buf = [0; 6];
            buf.copy_from_slice(bytes);

            Ok(HalConfigParameter::PublicAddress(crate::BdAddr(buf)))
        }
        2 => Ok(HalConfigParameter::Diversifier(LittleEndian::read_u16(
            bytes,
        ))),
        16 => {
            let mut buf = [0; 16];
            buf.copy_from_slice(bytes);

            Ok(HalConfigParameter::EncryptionKey(
                crate::host::EncryptionKey(buf),
            ))
        }
        1 => Ok(HalConfigParameter::Byte(bytes[0])),
        other => Err(crate::event::Error::Vendor(
            super::VendorError::BadConfigParameterLength(other),
        )),
    }
}

fn to_hal_tx_test_packet_count(bytes: &[u8]) -> Result<HalTxTestPacketCount, crate::event::Error> {
    decode_status_prefixed(bytes, 4)
}

impl TryFrom<&[u8]> for HalTxTestPacketCount {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_tx_test_packet_count(bytes)
    }
}

/// Parameters returned by the [HAL Get Link Status](crate::vendor::command::hal::HalGetLinkStatus) command.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HalLinkStatus {
    /// State of the client connections.
    pub clients: [ClientStatus; 8],
}

/// State of a client connection.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ClientStatus {
    /// Link state for the client.
    pub state: LinkState,

    /// Connection handle for the client
    pub conn_handle: crate::ConnectionHandle,
}

/// Potential states for a connection.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LinkState {
    /// Idle
    Idle,
    /// Advertising
    Advertising,
    /// Connected in peripheral role
    ConnectedAsPeripheral,
    /// Scanning
    Scanning,
    /// Reserved
    Reserved,
    /// Connected in primary role
    ConnectedAsPrimary,
    /// TX Test
    TxTest,
    /// RX Test
    RxTest,
}

impl TryFrom<u8> for LinkState {
    type Error = super::VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LinkState::Idle),
            1 => Ok(LinkState::Advertising),
            2 => Ok(LinkState::ConnectedAsPeripheral),
            3 => Ok(LinkState::Scanning),
            4 => Ok(LinkState::Reserved),
            5 => Ok(LinkState::ConnectedAsPrimary),
            6 => Ok(LinkState::TxTest),
            7 => Ok(LinkState::RxTest),
            _ => Err(super::VendorError::UnknownLinkState(value)),
        }
    }
}

fn to_hal_link_status(bytes: &[u8]) -> Result<HalLinkStatus, crate::event::Error> {
    require_len!(bytes, 25);

    let mut status = HalLinkStatus {
        clients: [ClientStatus {
            state: LinkState::Idle,
            conn_handle: crate::ConnectionHandle(0),
        }; 8],
    };

    for client in 0..8 {
        status.clients[client].state = bytes[1 + client]
            .try_into()
            .map_err(crate::event::Error::Vendor)?;
        status.clients[client].conn_handle = crate::ConnectionHandle(LittleEndian::read_u16(
            &bytes[9 + 2 * client..9 + 2 * (client + 1)],
        ));
    }

    Ok(status)
}

impl TryFrom<&[u8]> for HalLinkStatus {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_link_status(bytes)
    }
}

/// Parameters returned by the [HAL Get Anchor Period](crate::vendor::command::hal::HalGetAnchorPeriod)
/// command.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HalAnchorPeriod {
    /// Duration between the beginnings of sniff anchor points.
    pub anchor_interval: Duration,

    /// Maximum available size that can be allocated to a new connection slot.
    pub max_slot: Duration,
}

fn to_hal_anchor_period(bytes: &[u8]) -> Result<HalAnchorPeriod, crate::event::Error> {
    require_len!(bytes, 9);

    Ok(HalAnchorPeriod {
        anchor_interval: Duration::from_micros(
            625 * u64::from(LittleEndian::read_u32(&bytes[1..5])),
        ),
        max_slot: Duration::from_micros(625 * u64::from(LittleEndian::read_u32(&bytes[5..9]))),
    })
}

impl TryFrom<&[u8]> for HalAnchorPeriod {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_anchor_period(bytes)
    }
}

fn to_hal_pm_debug_info(bytes: &[u8]) -> Result<HalPmDebugInfo, crate::event::Error> {
    decode_status_prefixed(bytes, 3)
}

impl TryFrom<&[u8]> for HalPmDebugInfo {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_hal_pm_debug_info(bytes)
    }
}

fn to_gap_init(bytes: &[u8]) -> Result<GapInit, crate::event::Error> {
    require_len!(bytes, 7);

    Ok(GapInit {
        service_handle: AttributeHandle(LittleEndian::read_u16(&bytes[1..])),
        dev_name_handle: AttributeHandle(LittleEndian::read_u16(&bytes[3..])),
        appearance_handle: AttributeHandle(LittleEndian::read_u16(&bytes[5..])),
    })
}

impl TryFrom<&[u8]> for GapInit {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gap_init(bytes)
    }
}

/// Parameters returned by the [GAP Get Security Level](crate::vendor::command::gap::GapGetSecurityLevel)
/// command.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GapSecurityLevel {
    /// GAP security mode. STM32WB firmware currently reports `0x01` for
    /// Security Mode 1.
    pub security_mode: u8,

    /// GAP security level, from `0x01` (Level 1) through `0x04` (Level 4).
    pub security_level: u8,
}

impl TryFrom<&[u8]> for GapSecurityLevel {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gap_security_level(bytes)
    }
}

/// Options for pass key generation.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PassKeyRequirement {
    /// A pass key is not required.
    NotRequired,
    /// A fixed pin is present which is being used.
    FixedPin,
    /// Pass key required for pairing. An event will be generated when required.
    Generated,
}

impl TryFrom<u8> for PassKeyRequirement {
    type Error = super::VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PassKeyRequirement::NotRequired),
            0x01 => Ok(PassKeyRequirement::FixedPin),
            0x02 => Ok(PassKeyRequirement::Generated),
            _ => Err(super::VendorError::BadPassKeyRequirement(value)),
        }
    }
}

fn to_gap_security_level(bytes: &[u8]) -> Result<GapSecurityLevel, crate::event::Error> {
    require_len!(bytes, 3);

    Ok(GapSecurityLevel {
        security_mode: bytes[1],
        security_level: bytes[2],
    })
}

/// Parameters returned by the
/// [GAP Resolve Private Address](crate::vendor::command::gap::CmdGapResolvePrivateAddress) command.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GapResolvePrivateAddress {
    /// If the address was successfully resolved, the peer address is returned.  This value is
    /// `None` if the address could not be resolved.
    pub bd_addr: Option<crate::BdAddr>,
}

impl TryFrom<&[u8]> for GapResolvePrivateAddress {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gap_resolve_private_address(bytes)
    }
}

fn to_gap_resolve_private_address(
    bytes: &[u8],
) -> Result<GapResolvePrivateAddress, crate::event::Error> {
    let status = to_status(bytes)?;
    if status == crate::Status::Success {
        require_len!(bytes, 7);

        let mut addr = [0; 6];
        addr.copy_from_slice(&bytes[1..7]);

        Ok(GapResolvePrivateAddress {
            bd_addr: Some(crate::BdAddr(addr)),
        })
    } else {
        Ok(GapResolvePrivateAddress { bd_addr: None })
    }
}

impl TryFrom<&[u8]> for GapBondedDevices {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gap_bonded_devices(bytes)
    }
}

fn to_gap_bonded_devices(bytes: &[u8]) -> Result<GapBondedDevices, crate::event::Error> {
    let status = to_status(bytes)?;
    match status {
        crate::Status::Success => {
            const HEADER_LEN: usize = 2;
            const ADDR_LEN: usize = 7;

            require_len_at_least!(bytes, HEADER_LEN);
            let address_count = bytes[1] as usize;
            if address_count > GapBondedDevices::MAX_ADDRESSES
                || bytes.len() != HEADER_LEN + ADDR_LEN * address_count
            {
                return Err(crate::event::Error::Vendor(
                    super::VendorError::PartialBondedDeviceAddress,
                ));
            }

            let mut address_buffer =
                [crate::BdAddrType::Public(crate::BdAddr([0; 6])); GapBondedDevices::MAX_ADDRESSES];
            for (i, byte) in address_buffer.iter_mut().enumerate().take(address_count) {
                let index = HEADER_LEN + i * ADDR_LEN;
                let mut addr = [0; 6];
                addr.copy_from_slice(&bytes[(1 + index)..(7 + index)]);
                *byte = crate::to_bd_addr_type(bytes[index], crate::BdAddr(addr)).map_err(|e| {
                    crate::event::Error::Vendor(super::VendorError::BadBdAddrType(e.0))
                })?;
            }

            let addresses = crate::vendor::command::BoundedItems::from_array_prefix(
                address_buffer,
                address_count,
            )
            .map_err(|_| {
                crate::event::Error::Vendor(super::VendorError::PartialBondedDeviceAddress)
            })?;
            Ok(GapBondedDevices { addresses })
        }
        _ => {
            let addresses = crate::vendor::command::BoundedItems::from_array_prefix(
                [crate::BdAddrType::Public(crate::BdAddr([0; 6])); GapBondedDevices::MAX_ADDRESSES],
                0,
            )
            .map_err(|_| {
                crate::event::Error::Vendor(super::VendorError::PartialBondedDeviceAddress)
            })?;
            Ok(GapBondedDevices { addresses })
        }
    }
}

impl TryFrom<&[u8]> for GattService {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gatt_service(bytes)
    }
}

fn to_gatt_service(bytes: &[u8]) -> Result<GattService, crate::event::Error> {
    require_len!(bytes, 3);

    Ok(GattService {
        service_handle: AttributeHandle(LittleEndian::read_u16(&bytes[1..3])),
    })
}

fn to_gatt_characteristic(bytes: &[u8]) -> Result<GattCharacteristic, crate::event::Error> {
    require_len!(bytes, 3);

    Ok(GattCharacteristic {
        characteristic_handle: AttributeHandle(LittleEndian::read_u16(&bytes[1..3])),
    })
}

impl TryFrom<&[u8]> for GattCharacteristic {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gatt_characteristic(bytes)
    }
}

fn to_gatt_characteristic_descriptor(
    bytes: &[u8],
) -> Result<GattCharacteristicDescriptor, crate::event::Error> {
    require_len!(bytes, 3);

    Ok(GattCharacteristicDescriptor {
        descriptor_handle: AttributeHandle(LittleEndian::read_u16(&bytes[1..3])),
    })
}

impl TryFrom<&[u8]> for GattCharacteristicDescriptor {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gatt_characteristic_descriptor(bytes)
    }
}

fn to_gatt_handle_value(bytes: &[u8]) -> Result<GattHandleValue, crate::event::Error> {
    // ACI_GATT_READ_HANDLE_VALUE returns Status, Length, Value_Length, then Value.
    // `Length` is the total attribute length; this type intentionally exposes the requested
    // slice returned in `Value`.
    require_len_at_least!(bytes, 5);

    let value_len = LittleEndian::read_u16(&bytes[3..5]) as usize;
    require_len!(bytes, 5 + value_len);
    if value_len > GattHandleValue::MAX_VALUE_LEN {
        return Err(crate::event::Error::BadLength(
            value_len,
            GattHandleValue::MAX_VALUE_LEN,
        ));
    }

    <GattHandleValue as bt_hci::FromHciBytes>::from_hci_bytes_complete(&bytes[1..]).map_err(
        |error| match error {
            bt_hci::FromHciBytesError::InvalidSize => {
                crate::event::Error::BadLength(bytes.len(), 5 + value_len)
            }
            bt_hci::FromHciBytesError::InvalidValue => {
                crate::event::Error::BadLength(value_len, GattHandleValue::MAX_VALUE_LEN)
            }
        },
    )
}

impl TryFrom<&[u8]> for GattHandleValue {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        to_gatt_handle_value(bytes)
    }
}

/// Parameters returned by the
/// [L2CAP CoC Connect Confirm](crate::vendor::command::l2cap::L2CocConnectConfirm)
/// command.
///
/// CubeWB returns the number of created channels followed by exactly that many
/// channel indices. The generated API limits the count to five even though its
/// C receive buffer is allocated at the generic event-capacity limit.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct L2CapCocConnectConfirmResponse {
    /// Number of created channels.
    pub channel_number: u8,
    channel_index_list: [u8; Self::MAX_CHANNELS],
}

impl L2CapCocConnectConfirmResponse {
    /// Maximum number of channel indices that can be returned by this command.
    pub const MAX_CHANNELS: usize = 5;

    /// Channel indices reported by the controller.
    pub fn channel_indices(&self) -> &[u8] {
        &self.channel_index_list[..usize::from(self.channel_number)]
    }

    pub(crate) fn from_channel_indices(
        channel_indices: &[u8],
    ) -> Result<Self, crate::event::Error> {
        if channel_indices.len() > Self::MAX_CHANNELS {
            return Err(crate::event::Error::BadLength(
                channel_indices.len(),
                Self::MAX_CHANNELS,
            ));
        }
        let mut channel_index_list = [0; Self::MAX_CHANNELS];
        channel_index_list[..channel_indices.len()].copy_from_slice(channel_indices);
        Ok(Self {
            channel_number: channel_indices.len() as u8,
            channel_index_list,
        })
    }
}

fn to_l2cap_coc_connect_confirm_response(
    bytes: &[u8],
) -> Result<L2CapCocConnectConfirmResponse, crate::event::Error> {
    // Status + Channel_Number. `bytes` includes the Command Complete status;
    // declarative command return decoders receive only the bytes after it.
    require_len_at_least!(bytes, 2);
    let channel_number = usize::from(bytes[1]);
    if channel_number > L2CapCocConnectConfirmResponse::MAX_CHANNELS {
        return Err(crate::event::Error::BadLength(
            channel_number,
            L2CapCocConnectConfirmResponse::MAX_CHANNELS,
        ));
    }
    require_len!(bytes, 2 + channel_number);

    L2CapCocConnectConfirmResponse::from_channel_indices(&bytes[2..])
}

impl TryFrom<&[u8]> for L2CapCocConnectConfirmResponse {
    type Error = crate::event::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        // The event-facing conversion includes the Command Complete status,
        // unlike the generated declarative return decoder.
        to_l2cap_coc_connect_confirm_response(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gatt_read_handle_value_response() {
        // Status + total attribute length + returned value length + value.
        let value = to_gatt_handle_value(&[0x00, 0x34, 0x12, 0x03, 0x00, 0xAA, 0xBB, 0xCC])
            .expect("valid ACI_GATT_READ_HANDLE_VALUE response");

        assert_eq!(value.value(), [0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn rejects_oversized_gatt_read_handle_value_response() {
        let mut bytes = [0; 255];
        bytes[3] = 250;

        let err = to_gatt_handle_value(&bytes).expect_err("value cannot exceed response buffer");
        assert_eq!(
            err,
            crate::event::Error::BadLength(250, GattHandleValue::MAX_VALUE_LEN)
        );
    }

    #[test]
    fn parses_l2cap_coc_connect_confirm_response() {
        let response = to_l2cap_coc_connect_confirm_response(&[0x00, 2, 0xA1, 0xB2])
            .expect("valid L2CAP CoC Connect Confirm response");
        assert_eq!(response.channel_number, 2);
        assert_eq!(response.channel_indices(), [0xA1, 0xB2]);
    }
}
