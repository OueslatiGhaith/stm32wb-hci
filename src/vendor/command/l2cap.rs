//! L2Cap-specific commands and types needed for those commands.

extern crate byteorder;

use crate::{
    BadStatusError, ConnectionHandle, Status,
    types::{ConnectionInterval, ExpectedConnectionLength},
    vendor::command::ParamBuffer,
};
use bt_hci::{
    cmd::SyncCmd,
    controller::{ControllerCmdAsync, ControllerCmdSync},
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
        &self,
        params: &ConnectionParameterUpdateRequest,
    ) -> Result<(), Error>;

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
        &self,
        params: &ConnectionParameterUpdateResponse,
    ) -> Result<(), Error>;

    /// This command sends a Credit-Based Connection Request packet to the specified connection.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_connect(&self, params: &L2CapCocConnect) -> Result<(), Error>;

    /// This command sends a Credit-Based Connection Response packet. It must be used upon receipt
    /// of a connection request though [L2CAP COC Connection](crate::vendor::event::VendorEvent::L2CapCocConnect)
    /// event.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_connect_confirm(&self, params: &L2CapCocConnectConfirm) -> Result<(), Error>;

    /// This command sends a Credit-Based Reconfigure Request packet on the specified connection.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_reconfig(&self, params: &L2CapCocReconfig) -> Result<(), Error>;

    /// This command sends a Credit-Based Reconfigure Response packet. It must be use upon receipt
    /// of a Credit-Based Reconfigure Request through
    /// [L2CAP COC Reconfigure](crate::vendor::event::VendorEvent::L2CapCocReconfig) event.
    ///
    ///  See Bluetooth Core specification Vol.3 Part A.
    async fn coc_reconfig_confirm(&self, params: &L2CapCocReconfigConfirm) -> Result<(), Error>;

    /// This command sends a Disconnection Request signaling packet on the specified connection-oriented
    /// channel.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    ///
    /// # Generated events
    /// A [L2CAP COC Disconnection](crate::vendor::event::VendorEvent::L2CapCocDisconnect) event is
    /// received when the disconnection of the channel is effective.
    async fn coc_disconnect(&self, channel_index: u8) -> Result<(), Error>;

    /// This command sends a Flow Control Credit signaling packet on the specified connection-oriented
    /// channel.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    async fn coc_flow_control(&self, params: &L2CapCocFlowControl) -> Result<(), Error>;

    /// This command sends a K-frame packet on the specified connection-oriented channel.
    ///
    /// See Bluetooth Core specification Vol.3 Part A.
    ///
    /// # Note
    /// for the first K-frame of the SDU, the Information data shall contain
    /// the L2CAP SDU Length coded on two octets followed by the K-frame information
    /// payload. For the next K-frames of the SDU, the Information data shall only
    /// contain the K-frame information payload.
    /// The Length value must not exceed (BLE_CMD_MAX_PARAM_LEN - 3) i.e. 252 for
    /// BLE_CMD_MAX_PARAM_LEN default value.
    async fn coc_tx_data(&self, params: &L2CapCocTxData) -> Result<(), Error>;
}

vendor_cmd! {
    L2ConnectionParameterUpdateRequest(L2CAP_CONN_PARAM_UPDATE_REQ) {
        Params<'a> = ParamBuffer<'a>;
    }
}

vendor_cmd! {
    L2ConnectionParameterUpdateResponse(L2CAP_CONN_PARAM_UPDATE_RESP) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocConnect(L2CAP_COC_CONNECT) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocConnectConfirm(L2CAP_COC_CONNECT_CONFIRM) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocReconfig(L2CAP_COC_RECONFIG) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocReconfigConfirm(L2CAP_COC_RECONFIG_CONFIRM) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocDisconnect(L2CAP_COC_DISCONNECT) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocFlowControl(L2CAP_COC_DISCONNECT) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    L2CocTxData(L2CAP_COC_TX_DATA) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

impl<T> L2capCommands for T
where
    T: for<'t> ControllerCmdAsync<L2ConnectionParameterUpdateRequest<'t>>
        + for<'t> ControllerCmdSync<L2ConnectionParameterUpdateResponse<'t>>
        + for<'t> ControllerCmdSync<L2CocConnect<'t>>
        + for<'t> ControllerCmdSync<L2CocConnectConfirm<'t>>
        + for<'t> ControllerCmdSync<L2CocReconfig<'t>>
        + for<'t> ControllerCmdSync<L2CocReconfigConfirm<'t>>
        + for<'t> ControllerCmdSync<L2CocDisconnect<'t>>
        + for<'t> ControllerCmdSync<L2CocFlowControl<'t>>
        + for<'t> ControllerCmdSync<L2CocTxData<'t>>,
{
    hci_impl_params!(
        connection_parameter_update_request,
        ConnectionParameterUpdateRequest,
        L2ConnectionParameterUpdateRequest
    );

    hci_impl_params!(
        connection_parameter_update_response,
        ConnectionParameterUpdateResponse,
        L2ConnectionParameterUpdateResponse
    );

    hci_impl_params!(coc_connect, L2CapCocConnect, L2CocConnect);

    // TODO: This has a return buffer
    hci_impl_variable_length_params!(
        coc_connect_confirm,
        L2CapCocConnectConfirm,
        L2CocConnectConfirm
    );

    hci_impl_variable_length_params!(coc_reconfig, L2CapCocReconfig, L2CocReconfig);

    hci_impl_params!(
        coc_reconfig_confirm,
        L2CapCocReconfigConfirm,
        L2CocReconfigConfirm
    );

    async fn coc_disconnect(&self, channel_index: u8) -> Result<(), Error> {
        L2CocDisconnect::new((&[channel_index][..]).into())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    hci_impl_params!(coc_flow_control, L2CapCocFlowControl, L2CocFlowControl);

    hci_impl_variable_length_params!(coc_tx_data, L2CapCocTxData, L2CocTxData);
}

/// Potential errors from parameter validation.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
    }
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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// This event is generated when receiving a valid Credit Based Reconfigure Response packet.
///
/// See Bluetooth spec. v.5.4 [Vol 3, Part A].
pub struct L2CapCocReconfigConfirm {
    /// handle of the connection where this event occured.
    pub conn_handle: ConnectionHandle,
    /// This parameter indicates the outcome of the request. A value of 0x0000
    /// indicates success while a non zero value indicates the request is refused
    ///
    /// Values:
    /// - 0x0000 .. 0x000C
    pub result: u16,
}

impl L2CapCocReconfigConfirm {
    const LENGTH: usize = 4;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert_eq!(bytes.len(), Self::LENGTH);

        LittleEndian::write_u16(&mut bytes[0..], self.conn_handle.0);
        LittleEndian::write_u16(&mut bytes[2..], self.result);
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// This event is generated when receiving a valid Flow Control Credit signaling packet.
///
/// See Bluetooth spec. v.5.4 [Vol 3, Part A].
pub struct L2CapCocFlowControl {
    /// Index of the connection-oriented channel for which the primitive applies.
    pub channel_index: u8,
    /// Number of credits the receiving device can increment, corresponding to the
    /// number of K-frames that can be sent to the peer device sending Flow Control
    /// Credit packet.
    ///
    /// Values:
    /// - 0 .. 65535
    pub credits: u16,
}

impl L2CapCocFlowControl {
    const LENGTH: usize = 3;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert_eq!(bytes.len(), Self::LENGTH);

        bytes[0] = self.channel_index;
        LittleEndian::write_u16(&mut bytes[1..], self.credits);
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Parameter for the [coc_tx_data](L2capCommands::coc_tx_data) command
pub struct L2CapCocTxData {
    pub channel_index: u8,
    pub length: u16,
    pub data: [u8; 252],
}

impl L2CapCocTxData {
    const MAX_LENGTH: usize = 256;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert!(bytes.len() >= Self::MAX_LENGTH);

        bytes[0] = self.channel_index;
        LittleEndian::write_u16(&mut bytes[1..], self.length);
        bytes[3..].copy_from_slice(&self.data);
    }
}
