//! L2Cap-specific commands and types needed for those commands.

use crate::{
    BadStatusError, ConnectionHandle, Status,
    types::{ConnectionInterval, ExpectedConnectionLength},
    vendor::{command::BoundedBytes, event::command::L2CapCocConnectConfirmResponse},
};
use bt_hci::{
    cmd::{AsyncCmd, SyncCmd},
    controller::{ControllerCmdAsync, ControllerCmdSync},
};

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
    async fn coc_connect_confirm(
        &self,
        params: &L2CapCocConnectConfirm,
    ) -> Result<L2CapCocConnectConfirmResponse, Error>;

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
        Params = {
            conn_handle: ConnectionHandle => 2,
            conn_interval: ConnectionInterval => 8,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    L2ConnectionParameterUpdateResponse(L2CAP_CONN_PARAM_UPDATE_RESP) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length_range: ExpectedConnectionLength => 4,
            identifier: u8 => 1,
            accepted: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    L2CocConnect(L2CAP_COC_CONNECT) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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

vendor_cmd! {
    L2CocConnectConfirm(L2CAP_COC_CONNECT_CONFIRM) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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

vendor_cmd! {
    L2CocReconfig(L2CAP_COC_RECONFIG) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
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

vendor_cmd! {
    L2CocReconfigConfirm(L2CAP_COC_RECONFIG_CONFIRM) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            result: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    L2CocDisconnect(L2CAP_COC_DISCONNECT) {
        Params = {
            channel_index: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    L2CocFlowControl(L2CAP_COC_FLOW_CONTROL) {
        Params = {
            channel_index: u8 => 1,
            credits: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    L2CocTxData(L2CAP_COC_TX_DATA) {
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

impl<T> L2capCommands for T
where
    T: ControllerCmdAsync<L2ConnectionParameterUpdateRequest>
        + ControllerCmdSync<L2ConnectionParameterUpdateResponse>
        + ControllerCmdSync<L2CocConnect>
        + ControllerCmdSync<L2CocConnectConfirm>
        + for<'t> ControllerCmdSync<L2CocReconfig<'t>>
        + ControllerCmdSync<L2CocReconfigConfirm>
        + ControllerCmdSync<L2CocDisconnect>
        + ControllerCmdSync<L2CocFlowControl>
        + for<'t> ControllerCmdSync<L2CocTxData<'t>>,
{
    async fn connection_parameter_update_request(
        &self,
        params: &ConnectionParameterUpdateRequest,
    ) -> Result<(), Error> {
        L2ConnectionParameterUpdateRequest::new(params.conn_handle, params.conn_interval)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn connection_parameter_update_response(
        &self,
        params: &ConnectionParameterUpdateResponse,
    ) -> Result<(), Error> {
        L2ConnectionParameterUpdateResponse::new(
            params.conn_handle,
            params.conn_interval,
            params.expected_connection_length_range.clone(),
            params.identifier,
            params.accepted,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn coc_connect(&self, params: &L2CapCocConnect) -> Result<(), Error> {
        L2CocConnect::new(
            params.conn_handle,
            params.spsm,
            params.mtu,
            params.mps,
            params.initial_credits,
            params.channel_number,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn coc_connect_confirm(
        &self,
        params: &L2CapCocConnectConfirm,
    ) -> Result<L2CapCocConnectConfirmResponse, Error> {
        let response = L2CocConnectConfirm::new(
            params.conn_handle,
            params.mtu,
            params.mps,
            params.initial_credits,
            params.result,
        )
        .exec(self)
        .await
        .map_err(Error::from)?;
        L2CapCocConnectConfirmResponse::from_channel_indices(response.channel_indices.as_slice())
            .map_err(Error::from)
    }

    async fn coc_reconfig(&self, params: &L2CapCocReconfig) -> Result<(), Error> {
        let count = usize::from(params.channel_number);
        let channel_indices = params
            .channel_index_list
            .get(..count)
            .ok_or(Error::InvalidChannelCount(params.channel_number))?;
        L2CocReconfig::try_new(params.conn_handle, params.mtu, params.mps, channel_indices)
            .map_err(|_| Error::InvalidChannelCount(params.channel_number))?
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn coc_reconfig_confirm(&self, params: &L2CapCocReconfigConfirm) -> Result<(), Error> {
        L2CocReconfigConfirm::new(params.conn_handle, params.result)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn coc_disconnect(&self, channel_index: u8) -> Result<(), Error> {
        L2CocDisconnect::new(channel_index)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn coc_flow_control(&self, params: &L2CapCocFlowControl) -> Result<(), Error> {
        L2CocFlowControl::new(params.channel_index, params.credits)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn coc_tx_data(&self, params: &L2CapCocTxData) -> Result<(), Error> {
        let count = usize::from(params.length);
        let data = params
            .data
            .get(..count)
            .ok_or(Error::InvalidDataLength(params.length))?;
        L2CocTxData::try_new(params.channel_index, data)
            .map_err(|_| Error::InvalidDataLength(params.length))?
            .exec(self)
            .await
            .map_err(Error::from)
    }
}

/// Potential errors from parameter validation.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// The declared channel count exceeds the channel-index backing array.
    InvalidChannelCount(u8),

    /// The declared K-frame length exceeds the data backing array.
    InvalidDataLength(u16),

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

impl crate::vendor::command::HciEncodeField<8> for ConnectionInterval {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

impl crate::vendor::command::HciEncodeField<4> for ExpectedConnectionLength {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
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

    /// Number of channels created by the controller.
    ///
    /// This is an output field in CubeWB's generated C API and is therefore
    /// ignored when serializing this value for
    /// [`L2capCommands::coc_connect_confirm`]. It remains here because the
    /// same public type also represents the corresponding incoming vendor
    /// event.
    pub channel_number: u8,

    /// Channel indices created by the controller.
    ///
    /// Like [`Self::channel_number`], this is response/event data rather than
    /// command input and is not transmitted by `coc_connect_confirm`.
    pub channel_index_list: [u8; 246],
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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Parameter for the [coc_tx_data](L2capCommands::coc_tx_data) command
pub struct L2CapCocTxData {
    pub channel_index: u8,
    pub length: u16,
    pub data: [u8; 252],
}
