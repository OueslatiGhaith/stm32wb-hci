//! Address parameters shared by STM32WB vendor commands and events.

use bt_hci::param::{AddrKind, AdvFilterPolicy, BdAddr};

/// A Bluetooth address paired with its public or random address kind.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BdAddrType {
    /// Public address.
    Public(BdAddr),
    /// Random address.
    Random(BdAddr),
}

impl From<BdAddrType> for AddrKind {
    fn from(value: BdAddrType) -> Self {
        match value {
            BdAddrType::Public(_) => AddrKind::PUBLIC,
            BdAddrType::Random(_) => AddrKind::RANDOM,
        }
    }
}

impl From<BdAddrType> for BdAddr {
    fn from(value: BdAddrType) -> Self {
        match value {
            BdAddrType::Public(address) | BdAddrType::Random(address) => address,
        }
    }
}

/// An unrecognized Bluetooth address-kind byte.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BdAddrTypeError(pub u8);

/// Decode an address-kind byte and its associated address.
pub fn to_bd_addr_type(address_type: u8, address: BdAddr) -> Result<BdAddrType, BdAddrTypeError> {
    match address_type {
        0 => Ok(BdAddrType::Public(address)),
        1 => Ok(BdAddrType::Random(address)),
        _ => Err(BdAddrTypeError(address_type)),
    }
}

impl BdAddrType {
    fn hci_fields(&self) -> (u8, BdAddr) {
        match *self {
            Self::Public(address) => (0, address),
            Self::Random(address) => (1, address),
        }
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    BdAddrType => 7 {
        Fields = {
            kind: u8 => 1,
            address: BdAddr => 6,
        };
        Encode = |value| { value.hci_fields() };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Indicates the type of address used in advertising and initiating packets.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum OwnAddressType: u8 => 1 {
        /// Public Device Address.
        Public = 0x00,
        /// Random Device Address.
        Random = 0x01,
        /// Generate a resolvable private address, falling back to the public address.
        PrivateFallbackPublic = 0x02,
        /// Generate a resolvable private address, falling back to the random address.
        PrivateFallbackRandom = 0x03,
    }
}

impl From<OwnAddressType> for AddrKind {
    fn from(value: OwnAddressType) -> Self {
        match value {
            OwnAddressType::Public => AddrKind::PUBLIC,
            OwnAddressType::Random => AddrKind::RANDOM,
            OwnAddressType::PrivateFallbackPublic => AddrKind::RESOLVABLE_PRIVATE_OR_PUBLIC,
            OwnAddressType::PrivateFallbackRandom => AddrKind::RESOLVABLE_PRIVATE_OR_RANDOM,
        }
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Filter policy used for undirected advertising.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AdvertisingFilterPolicy: u8 => 1 {
        /// Process scan and connection requests from all devices.
        AllowConnectionAndScan = 0x00,
        /// Filter scan requests, but process connection requests from all devices.
        AllowConnectionWhiteListScan = 0x01,
        /// Filter connection requests, but process scan requests from all devices.
        WhiteListConnectionAllowScan = 0x02,
        /// Filter both scan and connection requests.
        WhiteListConnectionAndScan = 0x03,
    }
}

impl From<AdvertisingFilterPolicy> for AdvFilterPolicy {
    fn from(value: AdvertisingFilterPolicy) -> Self {
        match value {
            AdvertisingFilterPolicy::AllowConnectionAndScan => AdvFilterPolicy::Unfiltered,
            AdvertisingFilterPolicy::AllowConnectionWhiteListScan => AdvFilterPolicy::FilterScan,
            AdvertisingFilterPolicy::WhiteListConnectionAllowScan => AdvFilterPolicy::FilterConn,
            AdvertisingFilterPolicy::WhiteListConnectionAndScan => {
                AdvFilterPolicy::FilterConnAndScan
            }
        }
    }
}

/// Address type and value used to identify a peer device.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PeerAddrType {
    /// Public Device Address.
    PublicDeviceAddress(BdAddr),
    /// Random Device Address.
    RandomDeviceAddress(BdAddr),
    /// Public Identity Address corresponding to a resolvable private address.
    PublicIdentityAddress(BdAddr),
    /// Random Identity Address corresponding to a resolvable private address.
    RandomIdentityAddress(BdAddr),
}

impl From<PeerAddrType> for BdAddrType {
    fn from(value: PeerAddrType) -> Self {
        match value {
            PeerAddrType::PublicDeviceAddress(addr) | PeerAddrType::PublicIdentityAddress(addr) => {
                BdAddrType::Public(addr)
            }
            PeerAddrType::RandomDeviceAddress(addr) | PeerAddrType::RandomIdentityAddress(addr) => {
                BdAddrType::Random(addr)
            }
        }
    }
}

impl From<PeerAddrType> for bt_hci::param::BdAddr {
    fn from(value: PeerAddrType) -> Self {
        match value {
            PeerAddrType::PublicDeviceAddress(addr)
            | PeerAddrType::RandomDeviceAddress(addr)
            | PeerAddrType::PublicIdentityAddress(addr)
            | PeerAddrType::RandomIdentityAddress(addr) => Self(addr.0),
        }
    }
}

impl From<PeerAddrType> for AddrKind {
    fn from(value: PeerAddrType) -> Self {
        match value {
            PeerAddrType::PublicDeviceAddress(_) => AddrKind::PUBLIC,
            PeerAddrType::RandomDeviceAddress(_) => AddrKind::RANDOM,
            PeerAddrType::PublicIdentityAddress(_) => AddrKind::RESOLVABLE_PRIVATE_OR_PUBLIC,
            PeerAddrType::RandomIdentityAddress(_) => AddrKind::RESOLVABLE_PRIVATE_OR_RANDOM,
        }
    }
}

impl PeerAddrType {
    fn hci_fields(&self) -> (u8, BdAddr) {
        match *self {
            Self::PublicDeviceAddress(addr) => (0x00, addr),
            Self::RandomDeviceAddress(addr) => (0x01, addr),
            Self::PublicIdentityAddress(addr) => (0x02, addr),
            Self::RandomIdentityAddress(addr) => (0x03, addr),
        }
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    PeerAddrType => 7 {
        Fields = {
            kind: u8 => 1,
            address: BdAddr => 6,
        };
        Encode = |value| { value.hci_fields() };
    }
}

/// Decode an HCI peer-address type byte and its associated address.
pub fn to_peer_addr_type(
    address_type: u8,
    address: BdAddr,
) -> Result<PeerAddrType, BdAddrTypeError> {
    match AddrKind(address_type) {
        AddrKind::PUBLIC => Ok(PeerAddrType::PublicDeviceAddress(address)),
        AddrKind::RANDOM => Ok(PeerAddrType::RandomDeviceAddress(address)),
        AddrKind::RESOLVABLE_PRIVATE_OR_PUBLIC => Ok(PeerAddrType::PublicIdentityAddress(address)),
        AddrKind::RESOLVABLE_PRIVATE_OR_RANDOM => Ok(PeerAddrType::RandomIdentityAddress(address)),
        _ => Err(BdAddrTypeError(address_type)),
    }
}
