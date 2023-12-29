//! L2Cap-specific commands and types needed for those commands.

extern crate byteorder;

use crate::{
    types::{ConnectionInterval, ExpectedConnectionLength},
    ConnectionHandle, Controller,
};
use byteorder::{ByteOrder, LittleEndian};

/// L2Cap-specific commands.
pub trait L2capCommands {
    /// Send an L2CAP connection parameter update request from the peripheral to the central
    /// device.
    ///
    /// # Errors
    ///
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event on the receipt of the command and
    /// an [L2CAP Connection Update Response](crate::vendor::event::L2CapConnectionUpdateResponse) event when the master
    /// responds to the request (accepts or rejects).
    async fn connection_parameter_update_request(
        &mut self,
        params: &ConnectionParameterUpdateRequest,
    );

    /// This command should be sent in response to the
    /// [`L2CapConnectionUpdateResponse`](crate::vendor::event::L2CapConnectionUpdateResponse)
    /// event from the controller. The accept parameter has to be set to true if the connection
    /// parameters given in the event are acceptable.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::event::command::CommandComplete) event is generated.
    async fn connection_parameter_update_response(
        &mut self,
        params: &ConnectionParameterUpdateResponse,
    );

    /// This command sends a Credit-Based Connection Request packet to the specified connection.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_connect(&mut self, params: &L2CapCocConnect);

    /// This command sends a Credit-Based Connection Response packet. It must be used upon receipt
    /// of a connection request though [L2CAP COC Connection](crate::vendor::event::VendorEvent::L2CapCocConnect)
    /// event.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_connect_confirm(&mut self, params: &L2CapCocConnectConfirm);

    /// This command sends a Credit-Based Reconfigure Request packet on the specified connection.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_reconfig(&mut self, params: &L2CapCocReconfig);
}

impl<T: Controller> L2capCommands for T {
    impl_params!(
        connection_parameter_update_request,
        ConnectionParameterUpdateRequest,
        crate::vendor::opcode::L2CAP_CONN_PARAM_UPDATE_REQ
    );

    impl_params!(
        connection_parameter_update_response,
        ConnectionParameterUpdateResponse,
        crate::vendor::opcode::L2CAP_CONN_PARAM_UPDATE_RESP
    );

    impl_params!(
        coc_connect,
        L2CapCocConnect,
        crate::vendor::opcode::L2CAP_COC_CONNECT
    );

    impl_variable_length_params!(
        coc_connect_confirm,
        L2CapCocConnectConfirm,
        crate::vendor::opcode::L2CAP_COC_CONNECT_CONFIRM
    );

    impl_variable_length_params!(
        coc_reconfig,
        L2CapCocReconfig,
        crate::vendor::opcode::L2CAP_COC_RECONFIG
    );
}

/// Parameters for the
/// [`connection_parameter_update_request`](L2capCommands::connection_parameter_update_request)
/// command.
pub struct ConnectionParameterUpdateRequest {
    /// Connection handle of the link which the connection parameter update request has to be sent.
    pub conn_handle: crate::ConnectionHandle,

    /// Defines the range of the connection interval.
    pub conn_interval: ConnectionInterval,
}

impl ConnectionParameterUpdateRequest {
    const LENGTH: usize = 10;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert_eq!(bytes.len(), Self::LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        self.conn_interval.copy_into_slice(&mut bytes[2..10]);
    }
}

/// Parameters for the
/// [`connection_parameter_update_response`](L2capCommands::connection_parameter_update_response)
/// command.
pub struct ConnectionParameterUpdateResponse {
    /// [Connection handle](crate::vendor::event::L2CapConnectionUpdateRequest::conn_handle) received in the
    /// [`L2CapConnectionUpdateRequest`](crate::vendor::event::L2CapConnectionUpdateRequest)
    /// event.
    pub conn_handle: crate::ConnectionHandle,

    /// [Connection interval](crate::vendor::event::L2CapConnectionUpdateRequest::conn_interval) received in
    /// the
    /// [`L2CapConnectionUpdateRequest`](crate::vendor::event::L2CapConnectionUpdateRequest)
    /// event.
    pub conn_interval: ConnectionInterval,

    /// Expected length of connection event needed for this connection.
    pub expected_connection_length_range: ExpectedConnectionLength,

    /// [Identifier](crate::vendor::event::L2CapConnectionUpdateRequest::identifier) received in the
    /// [`L2CapConnectionUpdateRequest`](crate::vendor::event::L2CapConnectionUpdateRequest)
    /// event.
    pub identifier: u8,

    /// True if the parameters from the
    /// [event](crate::vendor::event::L2CapConnectionUpdateRequest) are acceptable.
    pub accepted: bool,
}

impl ConnectionParameterUpdateResponse {
    const LENGTH: usize = 16;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert_eq!(bytes.len(), Self::LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        self.conn_interval.copy_into_slice(&mut bytes[2..10]);
        self.expected_connection_length_range
            .copy_into_slice(&mut bytes[10..14]);
        bytes[14] = self.identifier;
        bytes[15] = self.accepted as u8;
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// This event is generated when receiving a valid Credit Based Connection
/// Request packet.
///
/// See Bluetooth spec. v.5.4 [Vol 3, Part A].
pub struct L2CapCocConnect {
    /// handle of the connection where this event occured.
    pub conn_handle: ConnectionHandle,
    /// Simplified Protocol/Service Multiplexer
    ///
    /// Values:
    /// - 0x0000 .. 0x00FF
    pub spsm: u16,
    /// Maximum Transmission Unit
    ///
    /// Values:
    /// - 23 .. 65535
    pub mtu: u16,
    /// Maximum Payload Size (in octets)
    ///
    /// Values:
    /// - 23 .. 248
    pub mps: u16,
    /// Number of K-frames that can be received on the created channel(s) by
    /// the L2CAP layer entity sending this packet.
    ///
    /// Values:
    /// - 0 .. 65535
    pub initial_credits: u16,
    /// Number of channels to be created. If this parameter is
    /// set to 0, it requests the creation of one LE credit based connection-
    /// oriented channel. Otherwise, it requests the creation of one or more
    /// enhanced credit based connection-oriented channels.
    ///
    /// Values:
    /// - 0 .. 5
    pub channel_number: u8,
}

impl L2CapCocConnect {
    const LENGTH: usize = 11;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert_eq!(bytes.len(), Self::LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        LittleEndian::write_u16(&mut bytes[2..], self.spsm);
        LittleEndian::write_u16(&mut bytes[4..], self.mtu);
        LittleEndian::write_u16(&mut bytes[6..], self.mps);
        LittleEndian::write_u16(&mut bytes[8..], self.initial_credits);
        bytes[10] = self.channel_number;
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// This event is generated when receiving a valid Credit Based Connection Response packet.
///
/// See Bluetooth spec. v.5.4 [Vol 3, Part A].
pub struct L2CapCocConnectConfirm {
    /// handle of the connection where this event occured.
    pub conn_handle: ConnectionHandle,
    /// Maximum Transmission Unit
    ///
    /// Values:
    /// - 23 .. 65535
    pub mtu: u16,
    /// Maximum Payload Size (in octets)
    ///
    /// Values:
    /// - 23 .. 248
    pub mps: u16,
    /// Number of K-frames that can be received on the created channel(s) by
    /// the L2CAP layer entity sending this packet.
    ///
    /// Values:
    /// - 0 .. 65535
    pub initial_credits: u16,
    /// This parameter indicates the outcome of the request. A value of 0x0000
    /// indicates success while a non zero value indicates the request is refused
    ///
    /// Values:
    /// - 0x0000 .. 0x000C
    pub result: u16,
    /// Number of channels to be created. If this parameter is
    /// set to 0, it requests the creation of one LE credit based connection-
    /// oriented channel. Otherwise, it requests the creation of one or more
    /// enhanced credit based connection-oriented channels.
    ///
    /// Values:
    /// - 0 .. 5
    pub channel_number: u8,
    /// List of channel indexes for which the primitives apply.
    pub channel_index_list: [u8; 246],
}

impl L2CapCocConnectConfirm {
    const MAX_LENGTH: usize = 258;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert!(bytes.len() >= Self::MAX_LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        LittleEndian::write_u16(&mut bytes[2..], self.mtu);
        LittleEndian::write_u16(&mut bytes[4..], self.mps);
        LittleEndian::write_u16(&mut bytes[6..], self.initial_credits);
        LittleEndian::write_u16(&mut bytes[8..], self.result);
        bytes[10] = self.channel_number;
        bytes[11..].copy_from_slice(&self.channel_index_list);
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// This event is generated when receiving a valid Credit Based Reconfigure Request packet.
///
/// See Bluetooth spec. v.5.4 [Vol 3, Part A].
pub struct L2CapCocReconfig {
    /// handle of the connection where this event occured.
    pub conn_handle: ConnectionHandle,
    /// Maximum Transmission Unit
    ///
    /// Values:
    /// - 23 .. 65535
    pub mtu: u16,
    /// Maximum Payload Size (in octets)
    ///
    /// Values:
    /// - 23 .. 248
    pub mps: u16,
    /// Number of channels to be created. If this parameter is
    /// set to 0, it requests the creation of one LE credit based connection-
    /// oriented channel. Otherwise, it requests the creation of one or more
    /// enhanced credit based connection-oriented channels.
    ///
    /// Values:
    /// - 0 .. 5
    pub channel_number: u8,
    /// List of channel indexes for which the primitives apply.
    pub channel_index_list: [u8; 246],
}

impl L2CapCocReconfig {
    const MAX_LENGTH: usize = 254;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert!(bytes.len() >= Self::MAX_LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        LittleEndian::write_u16(&mut bytes[2..], self.mtu);
        LittleEndian::write_u16(&mut bytes[4..], self.mps);
        bytes[6] = self.channel_number;
        bytes[7..].copy_from_slice(&self.channel_index_list);
    }
}
