//! L2Cap-specific commands and types needed for those commands.

use bt_hci::param::ConnHandle;

use crate::{
    types::{ConnectionInterval, ExpectedConnectionLength},
    vendor::command::BoundedBytes,
};

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
            identifier: u8 => 1,
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
            spsm: u16 => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            initial_credits: u16 => 2,
            channel_number: u8 => 1,
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
            mtu: u16 => 2,
            mps: u16 => 2,
            initial_credits: u16 => 2,
            result: u16 => 2,
        };
        Completion = CommandComplete;
        Return = L2CapCocConnectConfirmWire {
            channel_indices: BoundedBytes<5> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 5,
            },
        };
    }
}

#[cfg(since_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    L2CocConnectConfirm(cgid = 0x3, cid = 0x09) {
        Params = {
            conn_handle: ConnHandle => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            initial_credits: u16 => 2,
            result: u16 => 2,
            max_channel_number: u8 => 1,
        };
        Completion = CommandComplete;
        Return = L2CapCocConnectConfirmWire {
            channel_indices: BoundedBytes<5> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 5,
            },
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocReconfig(cgid = 0x3, cid = 0x0A) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            channel_indices: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
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
            result: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocDisconnect(cgid = 0x3, cid = 0x0C) {
        Params = {
            channel_index: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocFlowControl(cgid = 0x3, cid = 0x0D) {
        Params = {
            channel_index: u8 => 1,
            credits: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    L2CocTxData(cgid = 0x3, cid = 0x0E) {
        Params<'a> = {
            channel_index: u8 => 1,
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
