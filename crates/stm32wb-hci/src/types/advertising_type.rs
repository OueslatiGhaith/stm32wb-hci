//! Legacy advertising packet type used by STM32WB GAP commands.

use bt_hci::param::AdvKind;

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Advertising packet type selected when advertising is enabled.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AdvertisingType: u8 => 1 {
        /// Connectable undirected advertising.
        ConnectableUndirected = 0x00,
        /// Connectable high-duty-cycle directed advertising.
        ConnectableDirectedHighDutyCycle = 0x01,
        /// Scannable undirected advertising.
        ScannableUndirected = 0x02,
        /// Non-connectable undirected advertising.
        NonConnectableUndirected = 0x03,
        /// Connectable low-duty-cycle directed advertising.
        ConnectableDirectedLowDutyCycle = 0x04,
    }
}

impl From<AdvertisingType> for AdvKind {
    fn from(value: AdvertisingType) -> Self {
        match value {
            AdvertisingType::ConnectableUndirected => Self::AdvInd,
            AdvertisingType::ConnectableDirectedHighDutyCycle => Self::AdvDirectIndHigh,
            AdvertisingType::ScannableUndirected => Self::AdvScanInd,
            AdvertisingType::NonConnectableUndirected => Self::AdvNonconnInd,
            AdvertisingType::ConnectableDirectedLowDutyCycle => Self::AdvDirectIndLow,
        }
    }
}
