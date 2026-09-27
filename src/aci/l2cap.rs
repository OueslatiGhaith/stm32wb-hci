//! L2CAP commands: connection parameter updates and credit-based channels.
//!
//! `aci_l2cap_coc_connect_confirm` is not declared yet: it gains a
//! `Max_Channel_Number` parameter in 1.23.0.

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no L2CAP commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::vendor_command;

vendor_command! {
    /// Ask the central to update the connection parameters, with intervals
    /// in units of 1.25 ms and the supervision timeout in units of 10 ms.
    /// The latency was named `Slave_latency` before 1.17.0.
    aci_l2cap_connection_parameter_update_req => L2capConnectionParameterUpdateReq {
        connection_handle: ConnHandle,
        conn_interval_min: u16,
        conn_interval_max: u16,
        latency: u16,
        timeout_multiplier: u16,
    }
}

vendor_command! {
    /// Accept or reject a peripheral's connection parameter update request,
    /// echoing its parameters and `identifier`.
    aci_l2cap_connection_parameter_update_resp => L2capConnectionParameterUpdateResp {
        connection_handle: ConnHandle,
        conn_interval_min: u16,
        conn_interval_max: u16,
        latency: u16,
        timeout_multiplier: u16,
        minimum_ce_length: u16,
        maximum_ce_length: u16,
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

#[cfg(all(test, feature = "stack-full-extended"))]
mod tests {
    use bt_hci::WriteHci;
    use bt_hci::cmd::Cmd;

    use super::*;

    fn encode(command: &impl WriteHci) -> ([u8; 300], usize) {
        let mut buffer = [0; 300];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 300 - writer.len();
        (buffer, len)
    }

    #[test]
    fn renamed_members_encode_in_every_release() {
        let command =
            L2capConnectionParameterUpdateReq::new(ConnHandle::new(0x0801), 6, 12, 1, 400);
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [0x81, 0xFD, 10, 0x01, 0x08, 6, 0, 12, 0, 1, 0, 0x90, 0x01]
        );

        let command = L2capConnectionParameterUpdateResp::new(
            ConnHandle::new(0x0801),
            6,
            12,
            1,
            400,
            2,
            4,
            7,
            true,
        );
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0x82, 0xFD, 16]);
        assert_eq!(bytes[13..len], [2, 0, 4, 0, 7, 1]);
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
}
