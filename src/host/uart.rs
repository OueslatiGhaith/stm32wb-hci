//! Packet-oriented HCI helpers for transports that include the packet indicator byte.
//!
//! [`UartHci`] is implemented for any [`bt_hci::controller::Controller`]. It complements the
//! command traits in [`crate::host`] and [`crate::vendor::command`] by providing a polling entry
//! point for controller-to-host packets.

use crate::event::{Error as EventError, Event};
use bt_hci::{ControllerToHostPacket, controller::Controller};
use byteorder::{ByteOrder, LittleEndian};

const PACKET_TYPE_HCI_COMMAND: u8 = 0x01;
// const PACKET_TYPE_ACL_DATA: u8 = 0x02;
// const PACKET_TYPE_SYNC_DATA: u8 = 0x03;
#[allow(dead_code)]
const PACKET_TYPE_HCI_EVENT: u8 = 0x04;

/// Potential errors from reading packets from the controller.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// The host expected the controller to begin a packet, but the next byte is not a valid packet
    /// type byte. Contains the value of the byte.
    BadPacketType(u8),
    /// There was an error deserializing an event. Contains the underlying error.
    BLE(EventError),

    /// An error occurred during operation of the controller
    IoError,
}

/// Packet types that may be read from the controller.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Packet {
    // AclData(AclData),
    // SyncData(SyncData),
    /// The HCI Event Packet is used by the Controller to notify the Host when events
    /// occur. The event is specialized to support vendor-specific events.
    Event(crate::Event),
}

/// Header for HCI Commands.
pub struct CommandHeader {
    opcode: crate::opcode::Opcode,
    param_len: u8,
}

/// Trait for reading packets from the controller.
pub trait UartHci {
    /// Reads and returns a packet from the controller. Consumes exactly enough bytes to read the
    /// next packet including its header.
    ///
    /// # Errors
    ///
    /// - Returns [`Error::BadPacketType`] if the next byte is not a valid
    ///   packet type.
    /// - Returns [`Error::BLE`] if there is an error deserializing the
    ///   packet (such as a mismatch between the packet length and the expected length of the
    ///   event). See [`crate::event::Error`] for possible values of `e`.
    /// - Returns [`Error::IoError`] if there is an error reading from the
    ///   controller.
    async fn read_packet(&self) -> Result<Packet, Error>;
}

impl super::HciHeader for CommandHeader {
    const HEADER_LENGTH: usize = 4;

    fn new(opcode: crate::opcode::Opcode, param_len: usize) -> CommandHeader {
        CommandHeader {
            opcode,
            param_len: param_len as u8,
        }
    }

    fn copy_into_slice(&self, buffer: &mut [u8]) {
        buffer[0] = PACKET_TYPE_HCI_COMMAND;
        LittleEndian::write_u16(&mut buffer[1..=2], self.opcode.0);
        buffer[3] = self.param_len;
    }
}

impl<T> UartHci for T
where
    T: Controller,
{
    async fn read_packet(&self) -> Result<Packet, Error> {
        const MAX_EVENT_LENGTH: usize = 256;

        let mut packet = [0u8; MAX_EVENT_LENGTH];
        let pkt = <Self as Controller>::read(self, &mut packet)
            .await
            .map_err(|_| Error::IoError)?;

        match pkt {
            ControllerToHostPacket::Event(pkt) => Ok(Packet::Event(
                match Event::from_kind_and_payload(pkt.kind.0, pkt.data).map_err(Error::BLE) {
                    Ok(pkt) => Ok(pkt),
                    Err(err) => {
                        warn!(
                            "failed to parse pkt({}): {:x} {:x}",
                            pkt.data.len(),
                            pkt.kind.0,
                            pkt.data[..pkt.data.len().min(10)]
                        );

                        Err(err)
                    }
                }?,
            )),
            _ => Err(Error::BadPacketType(pkt.kind() as u8)),
        }
    }
}
