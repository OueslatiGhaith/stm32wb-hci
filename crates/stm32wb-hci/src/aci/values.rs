//! Types standing for the values the vendor commands and events document
//! for their parameters and return parameters.
//!
//! Each is declared with [`wire_values!`](crate::wire_values), and every
//! command using one checks at compile time, on every target, that each of
//! its values is one the catalog documents for that parameter on STM32WB. A
//! parameter documenting fewer values takes a narrower type, such as
//! [`ConnectableOwnAddressType`]. A value only some releases document may
//! exist only in them, or, like one only some profiles document, be left
//! out, as its type says. A return or event parameter
//! is declared as [`OrUnknown`](crate::wire::OrUnknown), which keeps the
//! values left out.

use crate::wire_values;

wire_values! {
    /// The address a device uses for its own advertising or scanning.
    pub enum OwnAddressType: u8 {
        /// The public device address.
        Public = 0x00,
        /// The static random address.
        StaticRandom = 0x01,
        /// A resolvable private address.
        ResolvablePrivate = 0x02,
        /// A non-resolvable private address.
        NonResolvablePrivate = 0x03,
    }
}

wire_values! {
    /// The address a device uses for a connection, which cannot be a
    /// non-resolvable private address.
    pub enum ConnectableOwnAddressType: u8 {
        /// The public device address.
        Public = 0x00,
        /// The static random address.
        StaticRandom = 0x01,
        /// A resolvable private address.
        ResolvablePrivate = 0x02,
    }
}

wire_values! {
    /// Whether an address, or identity address, is public or random.
    pub enum AddressType: u8 {
        /// A public (identity) address.
        Public = 0x00,
        /// A random (static identity) address.
        Random = 0x01,
    }
}

wire_values! {
    /// The type of legacy undirected advertising.
    pub enum AdvertisingType: u8 {
        /// `ADV_IND`: connectable and scannable.
        ConnectableUndirected = 0x00,
        /// `ADV_SCAN_IND`: scannable only.
        ScannableUndirected = 0x02,
        /// `ADV_NONCONN_IND`: neither connectable nor scannable.
        NonConnectableUndirected = 0x03,
    }
}

wire_values! {
    /// The type of legacy undirected advertising that accepts no connection.
    pub enum NonConnectableAdvertisingType: u8 {
        /// `ADV_SCAN_IND`: scannable only.
        ScannableUndirected = 0x02,
        /// `ADV_NONCONN_IND`: neither connectable nor scannable.
        NonConnectableUndirected = 0x03,
    }
}

wire_values! {
    /// Whether scanning requests the scan responses of advertisers.
    pub enum ScanType: u8 {
        /// Listen to advertisements only.
        Passive = 0x00,
        /// Send scan requests.
        Active = 0x01,
    }
}

wire_values! {
    /// The input and output capabilities pairing uses.
    pub enum IoCapability: u8 {
        /// `IO_CAP_DISPLAY_ONLY`.
        DisplayOnly = 0x00,
        /// `IO_CAP_DISPLAY_YES_NO`.
        DisplayYesNo = 0x01,
        /// `IO_CAP_KEYBOARD_ONLY`.
        KeyboardOnly = 0x02,
        /// `IO_CAP_NO_INPUT_NO_OUTPUT`.
        NoInputNoOutput = 0x03,
        /// `IO_CAP_KEYBOARD_DISPLAY`.
        KeyboardDisplay = 0x04,
    }
}

wire_values! {
    /// Whether the GAP layer uses (controller) privacy. Enabled is 0x02, not 1.
    pub enum Privacy: u8 {
        /// Privacy disabled.
        Disabled = 0x00,
        /// Privacy enabled.
        Enabled = 0x02,
    }
}

wire_values! {
    /// The security mode of a connection.
    pub enum SecurityMode: u8 {
        /// Security mode 1: encryption.
        Mode1 = 0x01,
        /// Security mode 2: data signing, before 1.17.0.
        #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0"))]
        Mode2 = 0x02,
    }
}

wire_values! {
    /// The security level of a connection in security mode 1.
    pub enum SecurityLevel: u8 {
        /// No security.
        Level1 = 0x01,
        /// Unauthenticated pairing with encryption.
        Level2 = 0x02,
        /// Authenticated pairing with encryption.
        Level3 = 0x03,
        /// Authenticated LE Secure Connections pairing with encryption.
        Level4 = 0x04,
    }
}

wire_values! {
    /// A GAP procedure. Terminating a procedure also documents `0x00`, for
    /// none, which is left out.
    pub enum GapProcedure: u8 {
        /// `GAP_LIMITED_DISCOVERY_PROC`.
        LimitedDiscovery = 0x01,
        /// `GAP_GENERAL_DISCOVERY_PROC`.
        GeneralDiscovery = 0x02,
        /// `GAP_AUTO_CONNECTION_ESTABLISHMENT_PROC`.
        AutoConnectionEstablishment = 0x08,
        /// `GAP_GENERAL_CONNECTION_ESTABLISHMENT_PROC`.
        GeneralConnectionEstablishment = 0x10,
        /// `GAP_SELECTIVE_CONNECTION_ESTABLISHMENT_PROC`.
        SelectiveConnectionEstablishment = 0x20,
        /// `GAP_DIRECT_CONNECTION_ESTABLISHMENT_PROC`.
        DirectConnectionEstablishment = 0x40,
        /// `GAP_OBSERVATION_PROC`.
        Observation = 0x80,
    }
}

wire_values! {
    /// How pairing ended.
    pub enum PairingStatus: u8 {
        /// Pairing succeeded.
        Success = 0x00,
        /// The security manager timed out.
        SmpTimeout = 0x01,
        /// Pairing failed, for the reason given with it.
        PairingFailed = 0x02,
        /// Encrypting the link failed.
        EncryptionFailed = 0x03,
    }
}

wire_values! {
    /// Why pairing failed, as the security manager reports it.
    pub enum PairingFailureReason: u8 {
        /// The user cancelled or could not enter the passkey, from 1.17.1.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0"
        )))]
        PasskeyEntryFailed = 0x01,
        /// The out-of-band data is not available.
        OobNotAvailable = 0x02,
        /// The devices cannot meet the authentication requirements.
        AuthenticationRequirements = 0x03,
        /// The confirm value does not match the one calculated.
        ConfirmValueFailed = 0x04,
        /// The device does not support pairing.
        PairingNotSupported = 0x05,
        /// The resulting key would be too short.
        EncryptionKeySize = 0x06,
        /// The device does not support the SMP command received.
        CommandNotSupported = 0x07,
        /// Pairing failed for another reason.
        UnspecifiedReason = 0x08,
        /// Pairing was attempted again too soon.
        RepeatedAttempts = 0x09,
        /// A command had invalid parameters.
        InvalidParameters = 0x0A,
        /// The DHKey check failed.
        DhKeyCheckFailed = 0x0B,
        /// The numeric comparison values do not match.
        NumericComparisonFailed = 0x0C,
        /// The key was rejected, from 1.17.1.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0"
        )))]
        KeyRejected = 0x0F,
        /// The device is busy with another pairing, from 1.23.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0",
            feature = "fw_1_22_0",
            feature = "fw_1_22_1"
        )))]
        Busy = 0x10,
    }
}

wire_values! {
    /// A warning from the wireless stack.
    pub enum WarningType: u8 {
        /// Recombining an L2CAP packet failed.
        L2capRecombinationFailure = 0x01,
        /// A GATT message from the peer was unexpected.
        GattUnexpectedPeerMessage = 0x02,
        /// The non-volatile memory is almost full.
        NvmAlmostFull = 0x03,
        /// Data received on a connection-oriented channel was too long.
        CocRxDataLengthTooLarge = 0x04,
        /// A connection-oriented channel was given a DCID already assigned,
        /// from 1.18.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3"
        )))]
        CocAlreadyAssignedDcid = 0x05,
        /// The peer requested an LTK unexpectedly, from 1.22.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0"
        )))]
        SmpUnexpectedLtkRequest = 0x06,
        /// No GATT bearer was allocated, from 1.23.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0",
            feature = "fw_1_22_0",
            feature = "fw_1_22_1"
        )))]
        GattBearerNotAllocated = 0x07,
    }
}

wire_values! {
    /// The state an enhanced ATT bearer changed to.
    pub enum EattBearerState: u8 {
        /// The bearer was created.
        Created = 0x00,
        /// The bearer was terminated.
        Terminated = 0x01,
        /// The bearer was reconfigured, from 1.23.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0",
            feature = "fw_1_22_0",
            feature = "fw_1_22_1"
        )))]
        Reconfigured = 0x02,
    }
}

wire_values! {
    /// The central's answer to a connection parameter update request.
    pub enum ConnectionUpdateResult: u16 {
        /// The central accepted the parameters.
        Accepted = 0x0000,
        /// The central rejected the parameters.
        Rejected = 0x0001,
    }
}

wire_values! {
    /// Where a value of the configuration data starts, with its length.
    /// Every command using one also checks the length is the one the catalog
    /// documents.
    pub enum ConfigDataOffset: u8 {
        /// `CONFIG_DATA_PUBLIC_ADDRESS_OFFSET`: the public address.
        PublicAddress = 0x00 => [u8; 6],
        /// `CONFIG_DATA_ER_OFFSET`: the encryption root key.
        EncryptionRoot = 0x08 => [u8; 16],
        /// `CONFIG_DATA_IR_OFFSET`: the identity root key.
        IdentityRoot = 0x18 => [u8; 16],
        /// `CONFIG_DATA_RANDOM_ADDRESS_OFFSET`: the static random address.
        StaticRandomAddress = 0x2E => [u8; 6],
        /// `CONFIG_DATA_GAP_ADD_REC_NBR_OFFSET`: the number of additional
        /// records of the GAP service, from 1.22.0. Earlier releases
        /// document the offset without its length.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0"
        )))]
        GapAdditionalRecordNumber = 0x34 => [u8; 1],
        /// `CONFIG_DATA_SC_KEY_TYPE_OFFSET`: whether Secure Connections uses
        /// the normal or the debug keys, from 1.17.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0"
        )))]
        ScKeyType = 0x35 => [u8; 1],
        /// `CONFIG_DATA_SMP_MODE_OFFSET`: the SMP mode.
        SmpMode = 0xB0 => [u8; 1],
        /// `CONFIG_DATA_LL_SCAN_CHAN_MAP_OFFSET`: the channels scanning uses,
        /// as an [`AdvChannelMap`](crate::aci::flags::AdvChannelMap).
        ScanChannelMap = 0xC0 => [u8; 1],
        /// `CONFIG_DATA_LL_BG_SCAN_MODE_OFFSET`: whether background scanning
        /// is enabled, from 1.16.0.
        #[cfg(not(feature = "fw_1_15_0"))]
        BackgroundScanMode = 0xC1 => [u8; 1],
        /// `CONFIG_DATA_LL_RPA_MODE_OFFSET`: how the link layer updates
        /// resolvable private addresses, from 1.21.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0"
        )))]
        RpaMode = 0xC3 => [u8; 1],
        /// `CONFIG_DATA_LL_MAX_DATA_EXT_OFFSET`: the largest data length
        /// extension, as the supported maximum TX octets, TX time, RX octets
        /// and RX time, each a `u16`, from 1.21.0 on the full stack.
        #[cfg(all(
            not(any(
                feature = "fw_1_15_0",
                feature = "fw_1_16_0",
                feature = "fw_1_17_0",
                feature = "fw_1_17_1",
                feature = "fw_1_17_2",
                feature = "fw_1_17_3",
                feature = "fw_1_18_0",
                feature = "fw_1_19_0",
                feature = "fw_1_19_1",
                feature = "fw_1_20_0"
            )),
            any(feature = "stack-full-extended", feature = "stack-full")
        ))]
        MaxDataLengthExtension = 0xD1 => [u8; 8],
    }
}

wire_values! {
    /// Where a value of the configuration data that can be read back starts,
    /// with its length.
    pub enum ReadableConfigDataOffset: u8 {
        /// `CONFIG_DATA_PUBLIC_ADDRESS_OFFSET`: the public address.
        PublicAddress = 0x00 => [u8; 6],
        /// `CONFIG_DATA_ER_OFFSET`: the encryption root key.
        EncryptionRoot = 0x08 => [u8; 16],
        /// `CONFIG_DATA_IR_OFFSET`: the identity root key.
        IdentityRoot = 0x18 => [u8; 16],
        /// `CONFIG_DATA_RANDOM_ADDRESS_OFFSET`: the static random address.
        StaticRandomAddress = 0x2E => [u8; 6],
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

    use super::*;
    use crate::wire::{
        HciWireType, OrUnknown, decodes_any, flags_documented, is_opaque, lengths_documented,
        values_documented,
    };

    #[test]
    fn values_encode_as_their_documented_byte() {
        let mut buffer = [0u8; 1];
        Privacy::Enabled.write_hci(&mut buffer[..]).unwrap();
        assert_eq!(buffer, [0x02]);
        assert_eq!(
            Privacy::from_hci_bytes_complete(&[0x02]).unwrap(),
            Privacy::Enabled
        );
        assert_eq!(
            Privacy::from_hci_bytes_complete(&[0x01]),
            Err(FromHciBytesError::InvalidValue)
        );
        assert_eq!(u8::from(IoCapability::KeyboardDisplay), 0x04);
        assert_eq!(<Privacy as HciWireType>::WIDTH, 1);
    }

    #[test]
    fn every_value_must_be_documented() {
        let privacy = &[(0, 0), (2, 2)][..];
        assert!(values_documented::<Privacy>(privacy));
        assert!(!values_documented::<bool>(privacy), "1 is undocumented");
        assert!(values_documented::<bool>(&[(0, 1)]));
        assert!(values_documented::<OwnAddressType>(&[(0, 3)]));
        assert!(!values_documented::<OwnAddressType>(&[(0, 2)]));
        assert!(values_documented::<ConnectableOwnAddressType>(&[(0, 3)]));
        assert!(!is_opaque::<Privacy>(), "undocumented member");
        assert!(values_documented::<u8>(&[]), "integers stand for no values");
        assert!(is_opaque::<u8>());
    }

    #[test]
    fn decoded_values_keep_the_undocumented_ones() {
        assert_eq!(
            OrUnknown::<SecurityLevel>::from_hci_bytes_complete(&[0x03]).unwrap(),
            OrUnknown::Known(SecurityLevel::Level3)
        );
        let mode = OrUnknown::<SecurityMode>::from_hci_bytes_complete(&[0x03]).unwrap();
        assert_eq!(mode, OrUnknown::Unknown(0x03), "undocumented");
        assert_eq!(mode.known(), None);
        assert_eq!(mode.to_raw(), 0x03);
        assert_eq!(OrUnknown::from(SecurityMode::Mode1).to_raw(), 0x01);
        assert_eq!(
            OrUnknown::<SecurityMode>::from_hci_bytes_complete(&[]),
            Err(FromHciBytesError::InvalidSize)
        );

        assert!(decodes_any::<OrUnknown<SecurityLevel>>());
        assert!(!decodes_any::<SecurityLevel>(), "decoding fails on 0x05");
        assert!(!decodes_any::<bool>());
        assert!(decodes_any::<u8>());
        assert!(decodes_any::<[u8; 16]>(), "stands for no values");
        assert!(
            values_documented::<OrUnknown<SecurityLevel>>(&[(1, 4)]),
            "its values are those of the type"
        );
        assert!(!values_documented::<OrUnknown<SecurityLevel>>(&[(1, 3)]));
        assert_eq!(<OrUnknown<SecurityLevel> as HciWireType>::WIDTH, 1);

        assert_eq!(
            OrUnknown::<ConnectionUpdateResult>::from_hci_bytes_complete(&[0x01, 0x00]).unwrap(),
            OrUnknown::Known(ConnectionUpdateResult::Rejected)
        );
        assert_eq!(
            OrUnknown::<ConnectionUpdateResult>::from_hci_bytes_complete(&[0x00, 0x01]).unwrap(),
            OrUnknown::Unknown(0x0100)
        );
        assert_eq!(
            OrUnknown::<bool>::from_hci_bytes_complete(&[0x01]).unwrap(),
            OrUnknown::Known(true)
        );
        assert_eq!(
            OrUnknown::<bool>::from_hci_bytes_complete(&[0x02]).unwrap(),
            OrUnknown::Unknown(0x02)
        );
        assert!(decodes_any::<OrUnknown<bool>>());
    }

    #[cfg(feature = "fw_1_24_0")]
    #[test]
    fn later_releases_have_the_values_they_add() {
        assert_eq!(
            <WarningType as HciWireType>::VALUES,
            Some(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07][..])
        );
        assert_eq!(
            OrUnknown::<WarningType>::from_hci_bytes_complete(&[0x07]).unwrap(),
            OrUnknown::Known(WarningType::GattBearerNotAllocated)
        );
        assert_eq!(
            OrUnknown::<PairingFailureReason>::from_hci_bytes_complete(&[0x10]).unwrap(),
            OrUnknown::Known(PairingFailureReason::Busy)
        );
        assert_eq!(
            OrUnknown::<SecurityMode>::from_hci_bytes_complete(&[0x02]).unwrap(),
            OrUnknown::Unknown(0x02),
            "mode 2 is gone"
        );
        assert_eq!(ConfigDataOffset::RpaMode.length(), 1);
        assert_eq!(
            ConfigDataOffset::try_from(0x34),
            Ok(ConfigDataOffset::GapAdditionalRecordNumber)
        );
    }

    #[cfg(all(feature = "fw_1_24_0", feature = "stack-full-extended"))]
    #[test]
    fn the_full_stack_has_the_offsets_only_it_documents() {
        assert_eq!(ConfigDataOffset::MaxDataLengthExtension.length(), 8);
    }

    #[cfg(all(feature = "fw_1_24_0", feature = "stack-light"))]
    #[test]
    fn other_profiles_lack_the_offsets_only_the_full_stack_documents() {
        assert!(ConfigDataOffset::try_from(0xD1).is_err());
        assert!(ConfigDataOffset::try_from(0xC3).is_ok());
    }

    #[cfg(feature = "fw_1_16_0")]
    #[test]
    fn earlier_releases_lack_the_values_added_later() {
        assert_eq!(
            <WarningType as HciWireType>::VALUES,
            Some(&[0x01, 0x02, 0x03, 0x04][..])
        );
        assert_eq!(
            OrUnknown::<WarningType>::from_hci_bytes_complete(&[0x05]).unwrap(),
            OrUnknown::Unknown(0x05)
        );
        assert_eq!(
            OrUnknown::<PairingFailureReason>::from_hci_bytes_complete(&[0x01]).unwrap(),
            OrUnknown::Unknown(0x01)
        );
        assert_eq!(
            OrUnknown::<SecurityMode>::from_hci_bytes_complete(&[0x02]).unwrap(),
            OrUnknown::Known(SecurityMode::Mode2)
        );
        assert!(values_documented::<SecurityMode>(&[(1, 2)]));
        assert!(!values_documented::<SecurityMode>(&[(1, 1)]));
        assert_eq!(ConfigDataOffset::BackgroundScanMode.length(), 1);
        for offset in [0x34, 0x35, 0xC3, 0xD1] {
            assert!(ConfigDataOffset::try_from(offset).is_err(), "{offset:#x}");
        }
    }

    #[test]
    fn offsets_give_their_documented_lengths() {
        assert_eq!(ConfigDataOffset::EncryptionRoot.length(), 16);
        assert_eq!(ReadableConfigDataOffset::StaticRandomAddress.length(), 6);
        let lengths = &[(0x00, 6), (0x08, 16), (0x18, 16), (0x2E, 6)][..];
        assert!(lengths_documented::<ReadableConfigDataOffset>(lengths));
        assert!(
            !lengths_documented::<ReadableConfigDataOffset>(&lengths[..3]),
            "the random address has no documented length"
        );
        assert!(
            !lengths_documented::<ReadableConfigDataOffset>(&[
                (0x00, 6),
                (0x08, 16),
                (0x18, 8),
                (0x2E, 6)
            ]),
            "another length"
        );
        assert!(lengths_documented::<Privacy>(&[]), "no lengths given");
        assert!(!flags_documented::<ConfigDataOffset>(0xFF));
        assert!(!is_opaque::<ReadableConfigDataOffset>());
    }
}
