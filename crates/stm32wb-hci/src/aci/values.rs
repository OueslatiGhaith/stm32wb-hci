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
        /// `GAP_PERIODIC_ADVERTISING_CONNECTION_PROC`, on STM32WBA from 1.10.0.
        #[cfg(feature = "wba_1_10_0")]
        PeriodicAdvertisingConnection = 0x04,
        /// `GAP_AUTO_CONNECTION_ESTABLISHMENT_PROC`.
        AutoConnectionEstablishment = 0x08,
        /// `GAP_GENERAL_CONNECTION_ESTABLISHMENT_PROC`, on STM32WBA before
        /// 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        GeneralConnectionEstablishment = 0x10,
        /// `GAP_SELECTIVE_CONNECTION_ESTABLISHMENT_PROC`, on STM32WBA before
        /// 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        SelectiveConnectionEstablishment = 0x20,
        /// `GAP_DIRECT_CONNECTION_ESTABLISHMENT_PROC`.
        DirectConnectionEstablishment = 0x40,
        /// `GAP_OBSERVATION_PROC`, on STM32WBA before 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        Observation = 0x80,
        /// `GAP_GENERIC_SCAN_PROC`, on STM32WBA from 1.10.0.
        #[cfg(feature = "wba_1_10_0")]
        GenericScan = 0xB0,
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
        /// The user cancelled or could not enter the passkey, from 1.17.1,
        /// and STM32CubeWBA 1.2.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1"
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
        /// The key was rejected, from 1.17.1, and STM32CubeWBA 1.2.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1"
        )))]
        KeyRejected = 0x0F,
        /// The device is busy with another pairing, from 1.23.0, and
        /// STM32CubeWBA 1.7.0.
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
            feature = "fw_1_22_1",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0",
            feature = "wba_1_6_0",
            feature = "wba_1_6_1"
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
        /// from 1.18.0, and
        /// STM32CubeWBA 1.2.0.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1"
        )))]
        CocAlreadyAssignedDcid = 0x05,
        /// The peer requested an LTK unexpectedly, from 1.22.0, and STM32CubeWBA
        /// 1.6.0.
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
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0"
        )))]
        SmpUnexpectedLtkRequest = 0x06,
        /// No GATT bearer was allocated, from 1.23.0, and STM32CubeWBA 1.7.0.
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
            feature = "fw_1_22_1",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0",
            feature = "wba_1_6_0",
            feature = "wba_1_6_1"
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
        /// The bearer was reconfigured, from 1.23.0, and STM32CubeWBA 1.7.0.
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
            feature = "fw_1_22_1",
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0",
            feature = "wba_1_6_0",
            feature = "wba_1_6_1"
        )))]
        Reconfigured = 0x02,
    }
}

wire_values! {
    /// What a link is doing. The reserved 0x04 is left out.
    pub enum LinkState: u8 {
        /// The link is idle.
        Idle = 0x00,
        /// Advertising.
        Advertising = 0x01,
        /// Connected in the peripheral role.
        ConnectedPeripheral = 0x02,
        /// Scanning.
        Scanning = 0x03,
        /// Connected in the central role.
        ConnectedCentral = 0x05,
        /// In the direct transmit test.
        TxTest = 0x06,
        /// In the direct receive test.
        RxTest = 0x07,
        /// Advertising with the additional beacon.
        AdvertisingWithAdditionalBeacon = 0x81,
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
    /// Why the host terminates a connection.
    pub enum DisconnectReason: u8 {
        /// Authentication failure.
        AuthenticationFailure = 0x05,
        /// The user terminated the connection.
        RemoteUserTerminated = 0x13,
        /// The device is low on resources.
        RemoteLowResources = 0x14,
        /// The device is powering off.
        RemotePowerOff = 0x15,
        /// The peer lacks a feature the connection needs.
        UnsupportedRemoteFeature = 0x1A,
        /// The connection parameters are unacceptable.
        UnacceptableConnectionParameters = 0x3B,
    }
}

wire_values! {
    /// Which lists to add devices to, and whether to clear them first.
    pub enum AddDevicesMode: u8 {
        /// Append to the resolving list.
        AppendResolvingList = 0x00,
        /// Clear and set the resolving list.
        SetResolvingList = 0x01,
        /// Append to the Filter Accept List.
        AppendFilterAcceptList = 0x02,
        /// Clear and set the Filter Accept List.
        SetFilterAcceptList = 0x03,
        /// Append to both lists.
        AppendBoth = 0x04,
        /// Clear and set both lists.
        SetBoth = 0x05,
    }
}

wire_values! {
    /// Whether the controller may fragment advertising or scan response
    /// data.
    pub enum FragmentPreference: u8 {
        /// The controller may fragment the data.
        MayFragment = 0x00,
        /// The controller should not fragment the data, or fragment it as
        /// little as it can.
        Minimize = 0x01,
    }
}

wire_values! {
    /// Which part of the advertising data of an advertising set a command
    /// carries.
    pub enum AdvDataOperation: u8 {
        /// An intermediate fragment.
        Intermediate = 0x00,
        /// The first fragment.
        First = 0x01,
        /// The last fragment.
        Last = 0x02,
        /// The complete data.
        Complete = 0x03,
        /// Unchanged data, only updating the advertising DID.
        Unchanged = 0x04,
    }
}

wire_values! {
    /// Which part of the scan response data of an advertising set a command
    /// carries.
    pub enum ScanRespDataOperation: u8 {
        /// An intermediate fragment.
        Intermediate = 0x00,
        /// The first fragment.
        First = 0x01,
        /// The last fragment.
        Last = 0x02,
        /// The complete data.
        Complete = 0x03,
    }
}

wire_values! {
    /// Whose scan and connection requests an advertising set processes.
    pub enum AdvFilterPolicy: u8 {
        /// Every device's.
        All = 0x00,
        /// Every device's connection requests, and the scan requests of the
        /// devices in the Filter Accept List.
        FilterScanRequests = 0x01,
        /// Every device's scan requests, and the connection requests of the
        /// devices in the Filter Accept List.
        FilterConnectionRequests = 0x02,
        /// Only those of the devices in the Filter Accept List.
        FilterAcceptList = 0x03,
    }
}

wire_values! {
    /// Whose scan and connection requests undirected connectable advertising
    /// processes.
    pub enum UndirectedAdvFilterPolicy: u8 {
        /// Every device's.
        All = 0x00,
        /// Only those of the devices in the Filter Accept List.
        FilterAcceptList = 0x03,
    }
}

wire_values! {
    /// The PHY of the secondary advertising channel. The LE Coded PHY is
    /// not supported on STM32WB.
    pub enum SecondaryAdvPhy: u8 {
        /// LE 1M.
        Le1M = 0x01,
        /// LE 2M.
        Le2M = 0x02,
    }
}

wire_values! {
    /// The host's answer to an authorization request.
    pub enum AuthorizationResponse: u8 {
        /// Authorize the peer.
        Authorize = 0x01,
        /// Reject the peer.
        Reject = 0x02,
    }
}

wire_values! {
    /// The connection procedure of an extended connection.
    pub enum ConnectionProcedure: u8 {
        /// Connect to any device of the Filter Accept List.
        AutoConnectionEstablishment = 0x08,
        /// Connect to one device.
        DirectConnectionEstablishment = 0x40,
    }
}

wire_values! {
    /// How the initiator selects the advertiser to connect to.
    pub enum InitiatorFilterPolicy: u8 {
        /// The given peer address.
        PeerAddress = 0x00,
        /// Any device of the Filter Accept List, ignoring the peer address.
        FilterAcceptList = 0x01,
    }
}

wire_values! {
    /// Whether an extended scan reports each advertiser once.
    pub enum FilterDuplicates: u8 {
        /// Report every advertisement.
        Disabled = 0x00,
        /// Report each advertiser once.
        Enabled = 0x01,
        /// Report each advertiser once per scan period, from 1.22.0, and
        /// STM32CubeWBA 1.6.0.
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
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0"
        )))]
        EnabledPerPeriod = 0x02,
    }
}

wire_values! {
    /// The GAP procedure of an extended scan.
    pub enum ScanProcedure: u8 {
        /// Limited discovery.
        LimitedDiscovery = 0x01,
        /// General discovery.
        GeneralDiscovery = 0x02,
        /// General connection establishment, on STM32WBA before 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        GeneralConnectionEstablishment = 0x10,
        /// Selective connection establishment, on STM32WBA before 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        SelectiveConnectionEstablishment = 0x20,
        /// Observation, on STM32WBA before 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        Observation = 0x80,
        /// A generic scan, on STM32WBA from 1.10.0.
        #[cfg(feature = "wba_1_10_0")]
        GenericScan = 0xB0,
    }
}

wire_values! {
    /// Which advertisements a scan accepts.
    pub enum ScanningFilterPolicy: u8 {
        /// Every advertisement, except directed advertising to another
        /// device.
        BasicUnfiltered = 0x00,
        /// Only advertisements from the Filter Accept List.
        BasicFiltered = 0x01,
        /// As [`BasicUnfiltered`](Self::BasicUnfiltered), also accepting
        /// directed advertising to a resolvable private address.
        ExtendedUnfiltered = 0x02,
        /// As [`BasicFiltered`](Self::BasicFiltered), also accepting
        /// directed advertising to a resolvable private address.
        ExtendedFiltered = 0x03,
    }
}

wire_values! {
    /// The kind of out-of-band pairing data.
    pub enum OobDataType: u8 {
        /// The temporary key of legacy pairing.
        TemporaryKey = 0x00,
        /// The random value of Secure Connections.
        Random = 0x01,
        /// The confirm value of Secure Connections.
        Confirm = 0x02,
    }
}

wire_values! {
    /// Whose out-of-band pairing data is set.
    pub enum OobDevice: u8 {
        /// The local device's; the address is not used.
        Local = 0x00,
        /// The peer's.
        Remote = 0x01,
    }
}

wire_values! {
    /// A keypress notification during passkey entry.
    pub enum PasskeyInputType: u8 {
        /// Passkey entry started.
        EntryStarted = 0x00,
        /// A digit was entered.
        DigitEntered = 0x01,
        /// A digit was erased.
        DigitErased = 0x02,
        /// The passkey was cleared.
        Cleared = 0x03,
        /// Passkey entry completed.
        EntryCompleted = 0x04,
    }
}

wire_values! {
    /// Whether pairing uses Secure Connections.
    pub enum ScSupport: u8 {
        /// Secure Connections pairing is not supported.
        NotSupported = 0x00,
        /// Secure Connections pairing is supported but optional.
        Optional = 0x01,
        /// Only Secure Connections pairing is accepted.
        Mandatory = 0x02,
    }
}

wire_values! {
    /// Whether pairing uses the fixed passkey. 1.24.0 deprecates it.
    pub enum UseFixedPin: u8 {
        /// Use the fixed passkey.
        Yes = 0x00,
        /// Ask the host for the passkey.
        No = 0x01,
    }
}

wire_values! {
    /// The kind of directed connectable advertising.
    pub enum DirectedAdvertisingType: u8 {
        /// High duty cycle directed advertising.
        HighDutyCycle = 0x01,
        /// Low duty cycle directed advertising.
        LowDutyCycle = 0x04,
    }
}

wire_values! {
    /// Whether a service is primary or secondary.
    pub enum ServiceType: u8 {
        /// A primary service.
        Primary = 0x01,
        /// A secondary service.
        Secondary = 0x02,
    }
}

wire_values! {
    /// The error an ATT server answered a request with.
    pub enum AttErrorCode: u8 {
        /// The attribute handle is invalid.
        InvalidHandle = 0x01,
        /// The attribute cannot be read.
        ReadNotPermitted = 0x02,
        /// The attribute cannot be written.
        WriteNotPermitted = 0x03,
        /// The PDU is invalid.
        InvalidPdu = 0x04,
        /// The attribute requires authentication.
        InsufficientAuthentication = 0x05,
        /// The server does not support the request.
        RequestNotSupported = 0x06,
        /// The offset is past the end of the attribute.
        InvalidOffset = 0x07,
        /// The attribute requires authorization.
        InsufficientAuthorization = 0x08,
        /// The prepare queue is full.
        PrepareQueueFull = 0x09,
        /// No attribute is in the handle range.
        AttributeNotFound = 0x0A,
        /// The attribute cannot be read with a blob request.
        AttributeNotLong = 0x0B,
        /// The encryption key is too short.
        InsufficientEncryptionKeySize = 0x0C,
        /// The value has an invalid length.
        InvalidAttributeValueLength = 0x0D,
        /// The request failed unexpectedly.
        UnlikelyError = 0x0E,
        /// The attribute requires encryption.
        InsufficientEncryption = 0x0F,
        /// The grouping attribute type is not supported.
        UnsupportedGroupType = 0x10,
        /// The server lacks the resources to answer.
        InsufficientResources = 0x11,
        /// The client's view of the database is out of sync, from 1.16.0.
        #[cfg(not(feature = "fw_1_15_0"))]
        DatabaseOutOfSync = 0x12,
        /// The value is not allowed, from 1.16.0.
        #[cfg(not(feature = "fw_1_15_0"))]
        ValueNotAllowed = 0x13,
    }
}

wire_values! {
    /// Whether the host allows a read or write the server asked it to
    /// approve.
    pub enum PermitStatus: u8 {
        /// Allow the access.
        Allowed = 0x00,
        /// Deny the access.
        Denied = 0x01,
    }
}

wire_values! {
    /// How a write with response is sent.
    pub enum WriteMode: u8 {
        /// Write a characteristic value or descriptor.
        Write = 0x00,
        /// Write a long characteristic value or descriptor.
        WriteLong = 0x01,
        /// Write a characteristic value reliably.
        ReliableWrite = 0x02,
    }
}

wire_values! {
    /// Whether a write without response is signed.
    pub enum SignedWriteMode: u8 {
        /// An unsigned write.
        Unsigned = 0x00,
        /// A signed write.
        Signed = 0x01,
    }
}

wire_values! {
    /// Whether to encrypt or decrypt Encrypted Advertising Data.
    pub enum EadMode: u8 {
        /// Encrypt.
        Encrypt = 0x00,
        /// Decrypt.
        Decrypt = 0x01,
    }
}

wire_values! {
    /// A radio activity.
    pub enum RadioState: u8 {
        /// Idle.
        Idle = 0x00,
        /// Advertising.
        Advertising = 0x01,
        /// A connection event in the peripheral role.
        PeripheralConnection = 0x02,
        /// Scanning.
        Scanning = 0x03,
        /// A connection event in the central role.
        CentralConnection = 0x05,
        /// The direct transmit test.
        TxTest = 0x06,
        /// The direct receive test.
        RxTest = 0x07,
    }
}

wire_values! {
    /// The address type of a peer, telling identity addresses resolved from
    /// a resolvable private address apart.
    pub enum PeerAddressType: u8 {
        /// A public device address.
        Public = 0x00,
        /// A random device address.
        Random = 0x01,
        /// A public identity address.
        PublicIdentity = 0x02,
        /// A random (static) identity address.
        RandomIdentity = 0x03,
    }
}

wire_values! {
    /// Whether a reset changes the stack options.
    pub enum ResetMode: u8 {
        /// Keep the stack options.
        KeepOptions = 0x00,
        /// Apply the given stack options.
        ChangeOptions = 0x01,
    }
}

wire_values! {
    /// The address type the controller uses in a standard HCI command.
    pub enum HciOwnAddressType: u8 {
        /// The public device address.
        Public = 0x00,
        /// The random device address.
        Random = 0x01,
        /// A resolvable private address if the resolving list has one,
        /// otherwise the public address.
        ResolvableOrPublic = 0x02,
        /// A resolvable private address if the resolving list has one,
        /// otherwise the random address.
        ResolvableOrRandom = 0x03,
    }
}

wire_values! {
    /// Which private key computes a Diffie-Hellman key.
    pub enum DhkeyPrivateKey: u8 {
        /// The generated private key.
        Generated = 0x00,
        /// The debug private key.
        Debug = 0x01,
    }
}

wire_values! {
    /// The payload of direct transmit test packets.
    pub enum TestPayload: u8 {
        /// The PRBS9 sequence.
        Prbs9 = 0x00,
        /// Alternating nibbles, `11110000`.
        Pattern11110000 = 0x01,
        /// Alternating bits, `10101010`.
        Pattern10101010 = 0x02,
        /// The PRBS15 sequence.
        Prbs15 = 0x03,
        /// Every bit 1.
        AllOnes = 0x04,
        /// Every bit 0.
        AllZeros = 0x05,
        /// Alternating nibbles, `00001111`.
        Pattern00001111 = 0x06,
        /// Alternating bits, `0101`.
        Pattern0101 = 0x07,
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
        /// records of the GAP service, from 1.22.0 on STM32WB and 1.6.0 on
        /// STM32WBA. Earlier releases document the offset without its
        /// length.
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
            feature = "wba_1_0_0",
            feature = "wba_1_1_0",
            feature = "wba_1_1_1",
            feature = "wba_1_2_0",
            feature = "wba_1_3_1",
            feature = "wba_1_4_0",
            feature = "wba_1_4_1",
            feature = "wba_1_5_0"
        )))]
        GapAdditionalRecordNumber = 0x34 => [u8; 1],
        /// `CONFIG_DATA_SC_KEY_TYPE_OFFSET`: whether Secure Connections uses
        /// the normal or the debug keys, from 1.17.0 on STM32WB and 1.1.0 on
        /// STM32WBA.
        #[cfg(not(any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "wba_1_0_0"
        )))]
        ScKeyType = 0x35 => [u8; 1],
        /// `CONFIG_DATA_SMP_MODE_OFFSET`: the SMP mode.
        SmpMode = 0xB0 => [u8; 1],
        /// `CONFIG_DATA_LL_SCAN_CHAN_MAP_OFFSET`: the channels scanning uses,
        /// as an [`AdvChannelMap`](crate::aci::flags::AdvChannelMap). STM32WB
        /// only.
        #[cfg(feature = "_stm32wb")]
        ScanChannelMap = 0xC0 => [u8; 1],
        /// `CONFIG_DATA_LL_BG_SCAN_MODE_OFFSET`: whether background scanning
        /// is enabled, on STM32WB from 1.16.0.
        #[cfg(all(feature = "_stm32wb", not(feature = "fw_1_15_0")))]
        BackgroundScanMode = 0xC1 => [u8; 1],
        /// `CONFIG_DATA_LL_RSSI_GOLDEN_RANGE_OFFSET`: the link layer's RSSI
        /// golden range, on STM32WBA from 1.5.0.
        #[cfg(all(
            feature = "_stm32wba",
            not(any(
                feature = "wba_1_0_0",
                feature = "wba_1_1_0",
                feature = "wba_1_1_1",
                feature = "wba_1_2_0",
                feature = "wba_1_3_1",
                feature = "wba_1_4_0",
                feature = "wba_1_4_1"
            ))
        ))]
        RssiGoldenRange = 0xC2 => [u8; 2],
        /// `CONFIG_DATA_LL_RPA_MODE_OFFSET`: how the link layer updates
        /// resolvable private addresses, on STM32WB from 1.21.0.
        #[cfg(all(
            feature = "_stm32wb",
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
            ))
        ))]
        RpaMode = 0xC3 => [u8; 1],
        /// `CONFIG_DATA_LL_RX_ACL_CTRL_OFFSET`: the link layer's control of
        /// received ACL data, on STM32WBA from 1.5.0.
        #[cfg(all(
            feature = "_stm32wba",
            not(any(
                feature = "wba_1_0_0",
                feature = "wba_1_1_0",
                feature = "wba_1_1_1",
                feature = "wba_1_2_0",
                feature = "wba_1_3_1",
                feature = "wba_1_4_0",
                feature = "wba_1_4_1"
            ))
        ))]
        RxAclControl = 0xC4 => [u8; 2],
        /// `CONFIG_DATA_LL_ISO_SCHED_MODE_OFFSET`: the link layer's
        /// isochronous scheduling mode, on STM32WBA from 1.10.0.
        #[cfg(feature = "wba_1_10_0")]
        IsoSchedulingMode = 0xC5 => [u8; 1],
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
