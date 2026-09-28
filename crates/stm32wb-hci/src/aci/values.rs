//! Types standing for the values the vendor commands document for their
//! parameters.
//!
//! Each is declared with [`wire_values!`](crate::wire_values), and every
//! command using one checks at compile time, on every target, that each of
//! its values is one the catalog documents for that parameter on STM32WB. A
//! parameter documenting fewer values takes a narrower type, such as
//! [`ConnectableOwnAddressType`].

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

#[cfg(test)]
mod tests {
    use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

    use super::*;
    use crate::wire::{HciWireType, is_opaque, values_documented};

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
}
