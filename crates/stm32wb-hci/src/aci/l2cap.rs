//! L2CAP commands and events: connection parameter updates and credit-based
//! channels.

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no L2CAP commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::{vendor_command, vendor_event};

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no L2CAP commands"
)]
use crate::aci::durations::{CeLength, ConnInterval};
#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no L2CAP commands"
)]
use crate::wire::BoundedBytes;

vendor_command! {
    /// Ask the central to update the connection parameters, with intervals
    /// in units of 1.25 ms and the supervision timeout in units of 10 ms.
    /// The latency was named `Slave_latency` before 1.17.0.
    aci_l2cap_connection_parameter_update_req => L2capConnectionParameterUpdateReq {
        connection_handle: ConnHandle,
        conn_interval_min: ConnInterval,
        conn_interval_max: ConnInterval,
        latency: u16,
        timeout_multiplier: u16,
    }
}

vendor_command! {
    /// Accept or reject a peripheral's connection parameter update request,
    /// echoing its parameters and `identifier`.
    aci_l2cap_connection_parameter_update_resp => L2capConnectionParameterUpdateResp {
        connection_handle: ConnHandle,
        conn_interval_min: ConnInterval,
        conn_interval_max: ConnInterval,
        latency: u16,
        timeout_multiplier: u16,
        minimum_ce_length: CeLength,
        maximum_ce_length: CeLength,
        identifier: u8,
        accept: bool,
    }
}

vendor_command! {
    /// Open `channel_number` credit-based channels to the protocol `spsm`.
    aci_l2cap_coc_connect => L2capCocConnect {
        connection_handle: ConnHandle,
        spsm: u16,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
        channel_number: u8,
    }
}

vendor_command! {
    /// Answer a peer's request to open credit-based channels, returning the
    /// indexes of the channels opened. From 1.23.0, `max_channel_number`
    /// limits how many channels are accepted.
    aci_l2cap_coc_connect_confirm => L2capCocConnectConfirm {
        connection_handle: ConnHandle,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
        result: u16,
        #[wire(since = "1.23.0")]
        max_channel_number: u8,
    } -> L2capCocChannels {
        channel_index_list: BoundedBytes<250>,
    }
}

vendor_command! {
    /// Change the MTU and MPS of the listed credit-based channels.
    aci_l2cap_coc_reconf => L2capCocReconf {
        connection_handle: ConnHandle,
        mtu: u16,
        mps: u16,
        channel_index_list: &'a [u8],
    }
}

vendor_command! {
    /// Answer a peer's credit-based channel reconfiguration.
    aci_l2cap_coc_reconf_confirm => L2capCocReconfConfirm {
        connection_handle: ConnHandle,
        result: u16,
    }
}

vendor_command! {
    /// Close a credit-based channel.
    aci_l2cap_coc_disconnect => L2capCocDisconnect {
        channel_index: u8,
    }
}

vendor_command! {
    /// Give the peer `credits` more frames on a credit-based channel.
    aci_l2cap_coc_flow_control => L2capCocFlowControl {
        channel_index: u8,
        credits: u16,
    }
}

vendor_command! {
    /// Send an SDU on a credit-based channel.
    aci_l2cap_coc_tx_data => L2capCocTxData {
        channel_index: u8,
        data: &'a [u8],
    }
}

vendor_event! {
    /// The peer answered a connection parameter update request.
    aci_l2cap_connection_update_resp_event => L2capConnectionUpdateRespEvent {
        connection_handle: ConnHandle,
        result: u16,
    }
}

vendor_event! {
    /// An L2CAP procedure timed out, with data depending on the procedure.
    aci_l2cap_proc_timeout_event => L2capProcTimeoutEvent {
        connection_handle: ConnHandle,
        data: &'a [u8],
    }
}

vendor_event! {
    /// The peer asked to update the connection parameters; answer with
    /// [`L2capConnectionParameterUpdateResp`].
    aci_l2cap_connection_update_req_event => L2capConnectionUpdateReqEvent {
        connection_handle: ConnHandle,
        identifier: u8,
        #[wire(name = "L2CAP_Length")]
        l2cap_length: u16,
        interval_min: u16,
        interval_max: u16,
        latency: u16,
        timeout_multiplier: u16,
    }
}

vendor_event! {
    /// The peer rejected an L2CAP command.
    aci_l2cap_command_reject_event => L2capCommandRejectEvent {
        connection_handle: ConnHandle,
        identifier: u8,
        reason: u16,
        data: &'a [u8],
    }
}

vendor_event! {
    /// The peer asked to open `channel_number` credit-based channels; answer
    /// with [`L2capCocConnectConfirm`].
    aci_l2cap_coc_connect_event => L2capCocConnectEvent {
        connection_handle: ConnHandle,
        spsm: u16,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
        channel_number: u8,
    }
}

vendor_event! {
    /// The peer answered a request to open credit-based channels.
    aci_l2cap_coc_connect_confirm_event => L2capCocConnectConfirmEvent {
        connection_handle: ConnHandle,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
        result: u16,
        channel_index_list: &'a [u8],
    }
}

vendor_event! {
    /// The peer asked to reconfigure credit-based channels; answer with
    /// [`L2capCocReconfConfirm`].
    aci_l2cap_coc_reconf_event => L2capCocReconfEvent {
        connection_handle: ConnHandle,
        mtu: u16,
        mps: u16,
        channel_index_list: &'a [u8],
    }
}

vendor_event! {
    /// The peer answered a request to reconfigure credit-based channels.
    aci_l2cap_coc_reconf_confirm_event => L2capCocReconfConfirmEvent {
        connection_handle: ConnHandle,
        result: u16,
    }
}

vendor_event! {
    /// A credit-based channel closed.
    aci_l2cap_coc_disconnect_event => L2capCocDisconnectEvent {
        channel_index: u8,
    }
}

vendor_event! {
    /// The peer granted credits on a credit-based channel.
    aci_l2cap_coc_flow_control_event => L2capCocFlowControlEvent {
        channel_index: u8,
        credits: u16,
    }
}

vendor_event! {
    /// Data arrived on a credit-based channel.
    aci_l2cap_coc_rx_data_event => L2capCocRxDataEvent {
        channel_index: u8,
        data: &'a [u8],
    }
}

vendor_event! {
    /// Buffers freed up after a send on a credit-based channel failed for
    /// lack of them.
    aci_l2cap_coc_tx_pool_available_event => L2capCocTxPoolAvailableEvent {}
}

#[cfg(all(test, feature = "stack-full-extended"))]
mod tests {
    use bt_hci::cmd::{Cmd, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;
    use crate::wire::VendorEvent;

    fn encode(command: &impl WriteHci) -> ([u8; 300], usize) {
        let mut buffer = [0; 300];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 300 - writer.len();
        (buffer, len)
    }

    #[test]
    fn renamed_members_encode_in_every_release() {
        let min = ConnInterval::MIN;
        let max = ConnInterval::from_millis(15).unwrap();
        let command =
            L2capConnectionParameterUpdateReq::new(ConnHandle::new(0x0801), min, max, 1, 400);
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [0x81, 0xFD, 10, 0x01, 0x08, 6, 0, 12, 0, 1, 0, 0x90, 0x01]
        );

        let command = L2capConnectionParameterUpdateResp::new(
            ConnHandle::new(0x0801),
            min,
            max,
            1,
            400,
            CeLength::from_units(2).unwrap(),
            CeLength::from_units(4).unwrap(),
            7,
            true,
        );
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0x82, 0xFD, 16]);
        assert_eq!(bytes[13..len], [2, 0, 4, 0, 7, 1]);
    }

    /// The command and its encoding in releases with `max_channel_number`.
    #[cfg(any(feature = "fw_1_23_0", feature = "fw_1_24_0"))]
    fn connect_confirm() -> (L2capCocConnectConfirm, &'static [u8]) {
        (
            L2capCocConnectConfirm::new(ConnHandle::new(1), 64, 32, 4, 0, 2),
            &[0x89, 0xFD, 11, 1, 0, 64, 0, 32, 0, 4, 0, 0, 0, 2],
        )
    }

    /// The command and its encoding in releases without `max_channel_number`.
    #[cfg(not(any(feature = "fw_1_23_0", feature = "fw_1_24_0")))]
    fn connect_confirm() -> (L2capCocConnectConfirm, &'static [u8]) {
        (
            L2capCocConnectConfirm::new(ConnHandle::new(1), 64, 32, 4, 0),
            &[0x89, 0xFD, 10, 1, 0, 64, 0, 32, 0, 4, 0, 0, 0],
        )
    }

    #[test]
    fn added_members_exist_from_their_release() {
        let (command, expected) = connect_confirm();
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], *expected);

        let channels =
            <L2capCocConnectConfirm as SyncCmd>::Return::from_hci_bytes_complete(&[2, 4, 5])
                .unwrap();
        assert_eq!(channels.channel_index_list.as_slice(), [4, 5]);
    }

    #[test]
    fn credit_based_channels_encode_their_lists() {
        let (bytes, len) =
            encode(&L2capCocReconf::try_new(ConnHandle::new(1), 64, 32, &[0, 2]).unwrap());
        assert_eq!(bytes[..len], [0x8A, 0xFD, 9, 1, 0, 64, 0, 32, 0, 2, 0, 2]);
        assert!(L2capCocReconf::try_new(ConnHandle::new(1), 64, 32, &[0; 249]).is_err());

        let (bytes, len) = encode(&L2capCocTxData::try_new(3, &[0xAB; 252]).unwrap());
        assert_eq!(bytes[..6], [0x8E, 0xFD, 255, 3, 252, 0]);
        assert_eq!(len, 3 + 255);
        assert!(L2capCocTxData::try_new(3, &[0; 253]).is_err());

        let (bytes, len) = encode(&L2capCocFlowControl::new(3, 10));
        assert_eq!(bytes[..len], [0x8D, 0xFD, 3, 3, 10, 0]);
        assert_eq!(L2capCocConnect::OPCODE.to_raw(), 0xFD88);
        assert_eq!(L2capCocReconfConfirm::OPCODE.to_raw(), 0xFD8B);
        assert_eq!(L2capCocDisconnect::OPCODE.to_raw(), 0xFD8C);
    }

    #[test]
    fn received_data_follows_a_16_bit_length() {
        let params = [0x16, 0x08, 3, 2, 0, 0xAA, 0xBB];
        let data = L2capCocRxDataEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        assert_eq!(L2capCocRxDataEvent::CODE, 0x0816);
        assert_eq!((data.channel_index, data.data), (3, &[0xAA, 0xBB][..]));
        assert!(
            L2capCocRxDataEvent::from_vendor_params(&params[..6])
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn channel_lists_follow_their_count() {
        let confirm = L2capCocConnectConfirmEvent::from_hci_bytes_complete(&[
            0x01, 0x00, 0x40, 0x00, 0x20, 0x00, 0x05, 0x00, 0x00, 0x00, 2, 7, 8,
        ])
        .unwrap();
        assert_eq!(confirm.connection_handle, ConnHandle::new(1));
        assert_eq!(
            (confirm.mtu, confirm.mps, confirm.initial_credits),
            (64, 32, 5)
        );
        assert_eq!(confirm.channel_index_list, [7, 8]);
    }

    #[test]
    fn update_requests_keep_the_renamed_latency() {
        let request = L2capConnectionUpdateReqEvent::from_hci_bytes_complete(&[
            0x01, 0x00, 9, 8, 0, 6, 0, 12, 0, 1, 0, 0xC8, 0,
        ])
        .unwrap();
        assert_eq!(request.identifier, 9);
        assert_eq!(request.l2cap_length, 8);
        assert_eq!((request.interval_min, request.interval_max), (6, 12));
        assert_eq!((request.latency, request.timeout_multiplier), (1, 200));
    }
}
