//! L2Cap-specific commands and types needed for those commands.

use bt_hci::param::ConnHandle;

use crate::{
    types::{ConnectionInterval, ExpectedConnectionLength},
    vendor::command::BoundedItems,
};

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    open_scalar
    /// Controller-assigned index identifying one LE credit-based channel.
    ///
    /// CubeWB does not publish a narrower numeric range; validity depends on
    /// the channels currently owned by the controller.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocChannelIndex: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    open_scalar
    /// L2CAP signaling identifier copied from the controller request event.
    ///
    /// The response command echoes an opaque controller-selected byte.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2SignalIdentifier: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Maximum transmission unit accepted by credit-based channel procedures.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocMtu: u16 => 2 {
        minimum: 23,
        maximum: u16::MAX,
    }
    EventError = |error| crate::vendor::event::VendorError::BadL2CocMtu(error).into();
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Maximum payload size accepted by credit-based channel procedures.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocMps: u16 => 2 {
        minimum: 23,
        maximum: 248,
    }
    EventError = |error| crate::vendor::event::VendorError::BadL2CocMps(error).into();
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Credit-based connection response result defined by the Bluetooth Core.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocConnectionResult: u16 => 2 {
        minimum: 0,
        maximum: 0x000F,
    }
    EventError = |error| crate::vendor::event::VendorError::BadL2CocConnectionResult(error).into();
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    open_scalar
    /// Initial receive-credit allocation for one or more credit-based channels.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocInitialCredits: u16 => 2;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Maximum number of credit-based channels accepted in one response.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocMaximumChannelCount: u8 => 1 {
        minimum: 1,
        maximum: 5,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Simplified Protocol/Service Multiplexer for a credit-based connection.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocSpsm: u16 => 2 {
        minimum: 1,
        maximum: 0x00FF,
    }
    EventError = |error| crate::vendor::event::VendorError::BadL2CocSpsm(error).into();
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Number of channels requested by a credit-based connection procedure.
    ///
    /// Zero requests one LE credit-based channel; one through five request
    /// that many enhanced credit-based channels.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocRequestedChannelCount: u8 => 1 {
        minimum: 0,
        maximum: 5,
    }
    EventError = |error| {
        crate::vendor::event::VendorError::BadL2CocRequestedChannelCount(error).into()
    };
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Credit-based reconfiguration response result defined by the Bluetooth Core.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocReconfigurationResult: u16 => 2 {
        minimum: 0,
        maximum: 0x0004,
    }
    EventError = |error| {
        crate::vendor::event::VendorError::BadL2CocReconfigurationResult(error).into()
    };
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    ranged
    /// Nonzero credit increment sent by a flow-control procedure.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct L2CocCreditIncrement: u16 => 2 {
        minimum: 1,
        maximum: u16::MAX,
    }
    EventError = |error| {
        crate::vendor::event::VendorError::BadL2CocCreditIncrement(error).into()
    };
}

stm32wb_hci_macros::vendor_cmd! {
    L2ConnectionParameterUpdateRequest(cgid = 0x3, cid = 0x01) {
        Params = {
            conn_handle: ConnHandle => 2,
            conn_interval: ConnectionInterval => 8,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2ConnectionParameterUpdateResponse(cgid = 0x3, cid = 0x02) {
        Params = {
            conn_handle: ConnHandle => 2,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length_range: ExpectedConnectionLength => 4,
            identifier: L2SignalIdentifier => 1,
            accepted: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocConnect(cgid = 0x3, cid = 0x08) {
        Params = {
            conn_handle: ConnHandle => 2,
            spsm: L2CocSpsm => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            // CubeWB documents the complete `u16` credit domain.
            initial_credits: L2CocInitialCredits => 2,
            channel_number: L2CocRequestedChannelCount => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    L2CocConnectConfirm(cgid = 0x3, cid = 0x09) {
        Params = {
            conn_handle: ConnHandle => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            // CubeWB documents the complete `u16` credit domain.
            initial_credits: L2CocInitialCredits => 2,
            result: L2CocConnectionResult => 2,
        };
        Completion = CommandComplete;
        Return = L2CapCocConnectConfirmWire {
            channel_indices: BoundedItems<L2CocChannelIndex, 5> => {
                kind: counted_items,
                count: u8 => 1,
                item: L2CocChannelIndex => 1,
                max_items: 5,
                storage_max_len: 251,
            },
        };
    }
}

#[cfg(since_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    L2CocConnectConfirm(cgid = 0x3, cid = 0x09) {
        Params = {
            conn_handle: ConnHandle => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            // CubeWB documents the complete `u16` credit domain.
            initial_credits: L2CocInitialCredits => 2,
            result: L2CocConnectionResult => 2,
            max_channel_number: L2CocMaximumChannelCount => 1,
        };
        Completion = CommandComplete;
        Return = L2CapCocConnectConfirmWire {
            channel_indices: BoundedItems<L2CocChannelIndex, 5> => {
                kind: counted_items,
                count: u8 => 1,
                item: L2CocChannelIndex => 1,
                max_items: 5,
                storage_max_len: 251,
            },
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocReconfig(cgid = 0x3, cid = 0x0A) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            channel_indices: &'a [L2CocChannelIndex] => {
                kind: counted_items,
                count: u8 => 1,
                item: L2CocChannelIndex => 1,
                min_items: 1,
                max_items: 5,
                storage_max_len: 249,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocReconfigConfirm(cgid = 0x3, cid = 0x0B) {
        Params = {
            conn_handle: ConnHandle => 2,
            result: L2CocReconfigurationResult => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocDisconnect(cgid = 0x3, cid = 0x0C) {
        Params = {
            channel_index: L2CocChannelIndex => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocFlowControl(cgid = 0x3, cid = 0x0D) {
        Params = {
            channel_index: L2CocChannelIndex => 1,
            credits: L2CocCreditIncrement => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocTxData(cgid = 0x3, cid = 0x0E) {
        Params<'a> = {
            channel_index: L2CocChannelIndex => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 252,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}
