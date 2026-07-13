//! L2Cap-specific commands and types needed for those commands.

use crate::{
    BadStatusError, ConnectionHandle, Status,
    types::{ConnectionInterval, ExpectedConnectionLength},
    vendor::command::BoundedBytes,
};

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
/// [`connection_parameter_update_request`](L2ConnectionParameterUpdateRequest)
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
/// [`connection_parameter_update_response`](L2ConnectionParameterUpdateResponse)
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
    /// [`L2CocConnectConfirm`]. It remains here because the
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
/// Parameter for the [coc_tx_data](L2CocTxData) command
pub struct L2CapCocTxData {
    pub channel_index: u8,
    pub length: u16,
    pub data: [u8; 252],
}
