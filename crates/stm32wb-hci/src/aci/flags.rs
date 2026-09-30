//! Types standing for the flags the vendor commands document for their
//! parameters.
//!
//! Each is declared with [`wire_flags!`](crate::wire_flags), and every
//! command using one checks at compile time, on every target, that each of
//! its flags is a bit the catalog documents for that parameter on STM32WB. A
//! flag only some releases document exists only in them.

use crate::wire_flags;

wire_flags! {
    /// The GAP roles a device takes.
    pub struct Role: u8 {
        /// Peripheral.
        const PERIPHERAL = 0x01;
        /// Broadcaster.
        const BROADCASTER = 0x02;
        /// Central.
        const CENTRAL = 0x04;
        /// Observer.
        const OBSERVER = 0x08;
    }
}

wire_flags! {
    /// The GAP events the host receives.
    pub struct GapEventMask: u16 {
        /// `ACI_GAP_LIMITED_DISCOVERABLE_EVENT`.
        const LIMITED_DISCOVERABLE = 0x0001;
        /// `ACI_GAP_PAIRING_COMPLETE_EVENT`.
        const PAIRING_COMPLETE = 0x0002;
        /// `ACI_GAP_PASS_KEY_REQ_EVENT`.
        const PASS_KEY_REQUEST = 0x0004;
        /// `ACI_GAP_AUTHORIZATION_REQ_EVENT`.
        const AUTHORIZATION_REQUEST = 0x0008;
        /// `ACI_GAP_PERIPHERAL_SECURITY_INITIATED_EVENT`, before 1.22.0.
        #[cfg(any(
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
        ))]
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// `ACI_GAP_BOND_LOST_EVENT`.
        const BOND_LOST = 0x0020;
        /// `ACI_GAP_PROC_COMPLETE_EVENT`.
        const PROCEDURE_COMPLETE = 0x0080;
        /// `ACI_L2CAP_CONNECTION_UPDATE_REQ_EVENT`.
        const L2CAP_CONNECTION_UPDATE_REQUEST = 0x0100;
        /// `ACI_L2CAP_CONNECTION_UPDATE_RESP_EVENT`.
        const L2CAP_CONNECTION_UPDATE_RESPONSE = 0x0200;
        /// `ACI_L2CAP_PROC_TIMEOUT_EVENT`.
        const L2CAP_PROCEDURE_TIMEOUT = 0x0400;
        /// `ACI_GAP_ADDR_NOT_RESOLVED_EVENT`.
        const ADDRESS_NOT_RESOLVED = 0x0800;
    }
}

wire_flags! {
    /// The primary advertising channels to use.
    pub struct AdvChannelMap: u8 {
        /// Channel 37.
        const CHANNEL_37 = 0x01;
        /// Channel 38.
        const CHANNEL_38 = 0x02;
        /// Channel 39.
        const CHANNEL_39 = 0x04;
    }
}

wire_flags! {
    /// How an advertising set is configured. Using LE Coded as the primary
    /// PHY is not supported on STM32WB.
    pub struct AdvMode: u8 {
        /// Advertise with the random address set for this advertising set.
        const SPECIFIC_RANDOM_ADDRESS = 0x01;
    }
}

wire_flags! {
    /// The properties of an advertising set's advertising events.
    pub struct AdvEventProperties: u16 {
        /// Connectable advertising.
        const CONNECTABLE = 0x0001;
        /// Scannable advertising.
        const SCANNABLE = 0x0002;
        /// Directed advertising.
        const DIRECTED = 0x0004;
        /// High duty cycle directed connectable advertising.
        const HIGH_DUTY_CYCLE_DIRECTED = 0x0008;
        /// Use legacy advertising PDUs.
        const LEGACY = 0x0010;
        /// Anonymous advertising.
        const ANONYMOUS = 0x0020;
        /// Include the transmit power in at least one advertising PDU.
        const INCLUDE_TX_POWER = 0x0040;
    }
}

wire_flags! {
    /// The PHYs to scan advertisements on. LE Coded is not supported on
    /// STM32WB.
    pub struct ScanningPhys: u8 {
        /// LE 1M.
        const LE_1M = 0x01;
    }
}

wire_flags! {
    /// The PHYs to connect with: LE 1M also scans connectable advertisements.
    /// LE Coded is not supported on STM32WB.
    pub struct InitiatingPhys: u8 {
        /// LE 1M.
        const LE_1M = 0x01;
        /// LE 2M.
        const LE_2M = 0x02;
    }
}

wire_flags! {
    /// The properties of a characteristic.
    pub struct CharProperties: u8 {
        /// `CHAR_PROP_BROADCAST`.
        const BROADCAST = 0x01;
        /// `CHAR_PROP_READ`.
        const READ = 0x02;
        /// `CHAR_PROP_WRITE_WITHOUT_RESP`.
        const WRITE_WITHOUT_RESPONSE = 0x04;
        /// `CHAR_PROP_WRITE`.
        const WRITE = 0x08;
        /// `CHAR_PROP_NOTIFY`.
        const NOTIFY = 0x10;
        /// `CHAR_PROP_INDICATE`.
        const INDICATE = 0x20;
        /// `CHAR_PROP_SIGNED_WRITE`: authenticated signed writes, which
        /// STM32WBA dropped in 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        const SIGNED_WRITE = 0x40;
        /// `CHAR_PROP_EXT`: extended properties.
        const EXTENDED_PROPERTIES = 0x80;
    }
}

wire_flags! {
    /// What reading or writing an attribute requires.
    pub struct SecurityPermissions: u8 {
        /// `AUTHEN_READ`.
        const AUTHENTICATED_READ = 0x01;
        /// `AUTHOR_READ`.
        const AUTHORIZED_READ = 0x02;
        /// `ENCRY_READ`.
        const ENCRYPTED_READ = 0x04;
        /// `AUTHEN_WRITE`.
        const AUTHENTICATED_WRITE = 0x08;
        /// `AUTHOR_WRITE`.
        const AUTHORIZED_WRITE = 0x10;
        /// `ENCRY_WRITE`.
        const ENCRYPTED_WRITE = 0x20;
        /// `SC_READ`: reading requires a Secure Connections link, from 1.24.0.
        #[cfg(feature = "fw_1_24_0")]
        const SECURE_CONNECTIONS_READ = 0x40;
        /// `SC_WRITE`: writing requires a Secure Connections link, from
        /// 1.24.0.
        #[cfg(feature = "fw_1_24_0")]
        const SECURE_CONNECTIONS_WRITE = 0x80;
    }
}

wire_flags! {
    /// How a client may access an attribute.
    pub struct AccessPermissions: u8 {
        /// `READ`.
        const READ = 0x01;
        /// `WRITE`.
        const WRITE = 0x02;
        /// `WRITE_WO_RESP`.
        const WRITE_WITHOUT_RESPONSE = 0x04;
        /// `SIGNED_WRITE`, which STM32WBA dropped in 1.10.0.
        #[cfg(not(feature = "wba_1_10_0"))]
        const SIGNED_WRITE = 0x08;
    }
}

wire_flags! {
    /// The GATT events a characteristic raises.
    pub struct GattEventMask: u8 {
        /// `GATT_NOTIFY_ATTRIBUTE_WRITE`.
        const ATTRIBUTE_WRITE = 0x01;
        /// `GATT_NOTIFY_WRITE_REQ_AND_WAIT_FOR_APPL_RESP`.
        const WRITE_REQUEST_AND_WAIT_FOR_RESPONSE = 0x02;
        /// `GATT_NOTIFY_READ_REQ_AND_WAIT_FOR_APPL_RESP`.
        const READ_REQUEST_AND_WAIT_FOR_RESPONSE = 0x04;
        /// `GATT_NOTIFY_NOTIFICATION_COMPLETION`, from 1.17.0, and
        /// STM32CubeWBA 1.1.0.
        #[cfg(not(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "wba_1_0_0")))]
        const NOTIFICATION_COMPLETION = 0x08;
    }
}

wire_flags! {
    /// The GATT events a characteristic descriptor raises. Unlike a
    /// characteristic, it cannot report that a notification completed.
    pub struct GattDescEventMask: u8 {
        /// `GATT_NOTIFY_ATTRIBUTE_WRITE`.
        const ATTRIBUTE_WRITE = 0x01;
        /// `GATT_NOTIFY_WRITE_REQ_AND_WAIT_FOR_APPL_RESP`.
        const WRITE_REQUEST_AND_WAIT_FOR_RESPONSE = 0x02;
        /// `GATT_NOTIFY_READ_REQ_AND_WAIT_FOR_APPL_RESP`.
        const READ_REQUEST_AND_WAIT_FOR_RESPONSE = 0x04;
    }
}

wire_flags! {
    /// How an updated characteristic value reaches the client; empty to
    /// notify no one.
    pub struct UpdateType: u8 {
        /// Notification.
        const NOTIFICATION = 0x01;
        /// Indication.
        const INDICATION = 0x02;
    }
}

wire_flags! {
    /// The radio activities the radio activity event reports.
    pub struct RadioActivityMask: u16 {
        /// Idle.
        const IDLE = 0x0001;
        /// Advertising.
        const ADVERTISING = 0x0002;
        /// Peripheral connection.
        const PERIPHERAL_CONNECTION = 0x0004;
        /// Scanning.
        const SCANNING = 0x0008;
        /// Central connection.
        const CENTRAL_CONNECTION = 0x0020;
        /// TX test mode.
        const TX_TEST_MODE = 0x0040;
        /// RX test mode.
        const RX_TEST_MODE = 0x0080;
    }
}

wire_flags! {
    /// The HAL events the host receives.
    pub struct HalEventMask: u32 {
        /// `ACI_HAL_SCAN_REQ_REPORT_EVENT`. STM32WB only.
        #[cfg(feature = "_stm32wb")]
        const SCAN_REQUEST_REPORT = 0x0000_0001;
        /// `ACI_HAL_SYNC_EVENT`. STM32WBA only.
        #[cfg(feature = "_stm32wba")]
        const SYNC = 0x0000_0002;
    }
}

wire_flags! {
    /// The options the wireless binary restarts with.
    pub struct ResetOptions: u32 {
        /// Link layer only mode.
        const LL_ONLY = 0x0000_0001;
        /// No service change description.
        const NO_SERVICE_CHANGE_DESCRIPTION = 0x0000_0002;
        /// The device name is read-only.
        const DEVICE_NAME_READ_ONLY = 0x0000_0004;
        /// Support extended advertising.
        const EXTENDED_ADVERTISING = 0x0000_0008;
        /// Support channel selection algorithm #2. STM32WB only.
        #[cfg(feature = "_stm32wb")]
        const CHANNEL_SELECTION_ALGORITHM_2 = 0x0000_0010;
        /// Reduced GATT database in NVM.
        const REDUCED_GATT_DATABASE = 0x0000_0020;
        /// Support GATT caching.
        const GATT_CACHING = 0x0000_0040;
        /// Support LE Power Class 1 (not available in RCP mode).
        const LE_POWER_CLASS_1 = 0x0000_0080;
        /// The appearance is writable.
        const APPEARANCE_WRITABLE = 0x0000_0100;
        /// Support enhanced ATT.
        const ENHANCED_ATT = 0x0000_0200;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::format;

    use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

    use super::*;
    use crate::aci::values::{AddressType, Privacy};
    use crate::wire::{HciWireType, OrUnknown, flags_documented, is_opaque, values_documented};

    #[test]
    fn flags_encode_as_their_bits() {
        let role = Role::PERIPHERAL | Role::CENTRAL;
        let mut buffer = [0u8; 2];
        role.write_hci(&mut buffer[..1]).unwrap();
        assert_eq!(buffer[0], 0x05);
        assert_eq!(Role::from_hci_bytes_complete(&[0x05]).unwrap(), role);
        assert_eq!(
            Role::from_hci_bytes_complete(&[0x10]),
            Err(FromHciBytesError::InvalidValue)
        );
        RadioActivityMask::all().write_hci(&mut buffer[..]).unwrap();
        assert_eq!(buffer, [0xEF, 0x00]);
        assert_eq!(<RadioActivityMask as HciWireType>::WIDTH, 2);
        assert!(role.contains(Role::CENTRAL) && !role.contains(Role::OBSERVER));
        assert_eq!(format!("{role:?}").as_str(), "Role(PERIPHERAL | CENTRAL)");
        assert_eq!(format!("{:?}", Role::empty()).as_str(), "Role(empty)");
        assert_eq!(
            OrUnknown::<Role>::from_hci_bytes_complete(&[0x15]).unwrap(),
            OrUnknown::Unknown(0x15),
            "a fallback keeps undocumented bits"
        );
    }

    #[test]
    fn every_flag_must_be_documented() {
        assert!(flags_documented::<Role>(0x0F));
        assert!(!flags_documented::<Role>(0x07), "observer is undocumented");
        assert!(flags_documented::<bool>(0x01), "a bool sets bit 0 or none");
        assert!(!flags_documented::<Privacy>(0x01), "privacy sets bit 1");
        assert!(flags_documented::<AddressType>(0x01));
        assert!(
            !values_documented::<Role>(&[(0, 0x0F)]),
            "a value list names no combinations"
        );
        assert!(!is_opaque::<Role>());
        assert!(flags_documented::<u8>(0), "integers stand for no flags");
    }

    #[cfg(feature = "fw_1_24_0")]
    #[test]
    fn later_releases_have_the_flags_they_add() {
        let permissions = SecurityPermissions::SECURE_CONNECTIONS_READ;
        assert_eq!(SecurityPermissions::all().bits(), 0xFF);
        assert_eq!(GattEventMask::all().bits(), 0x0F);
        assert_eq!(GapEventMask::all().bits(), 0x0FAF);
        assert_eq!(GapEventMask::from_bits(0x0010), None);
        assert_eq!(
            format!("{permissions:?}").as_str(),
            "SecurityPermissions(SECURE_CONNECTIONS_READ)"
        );
    }

    #[cfg(feature = "fw_1_16_0")]
    #[test]
    fn earlier_releases_lack_the_flags_added_later() {
        assert_eq!(SecurityPermissions::all().bits(), 0x3F);
        assert_eq!(GattEventMask::all().bits(), 0x07);
        assert_eq!(GapEventMask::all().bits(), 0x0FBF);
        assert_eq!(GattEventMask::from_bits(0x08), None);
        assert!(!flags_documented::<GapEventMask>(0x0FAF));
    }
}
