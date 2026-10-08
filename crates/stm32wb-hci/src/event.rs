//! Decoding the events the BLE stack sends: the Bluetooth Core's with
//! bt-hci, and ST's vendor events with [`AciEvent`].

use bt_hci::FromHciBytesError;
use bt_hci::event::{Event, EventKind, EventPacket};

use crate::aci::AciEvent;

/// An event the BLE stack sent.
///
/// ```ignore
/// let ControllerToHostPacket::Event(packet) = controller.read(&mut buf).await? else {
///     return Ok(());
/// };
/// match BleEvent::from_packet(packet)? {
///     BleEvent::Vendor(AciEvent::GapPairingComplete(pairing)) => on_pairing(pairing),
///     BleEvent::Core(Event::DisconnectionComplete(disconnection)) => on_disconnection(disconnection),
///     _ => {}
/// }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BleEvent<'a> {
    /// A Bluetooth Core event, or a vendor event the selected target does
    /// not emit, as bt-hci decodes it.
    Core(Event<'a>),
    /// An ST vendor event the selected target emits.
    Vendor(AciEvent<'a>),
}

impl<'a> BleEvent<'a> {
    /// Decode an event packet: a vendor event the selected target emits
    /// into its [`AciEvent`] variant, and any other into bt-hci's [`Event`].
    pub fn from_packet(packet: EventPacket<'a>) -> Result<Self, FromHciBytesError> {
        if packet.kind == EventKind::Vendor
            && let Some(event) = AciEvent::from_vendor_params(packet.data)
        {
            return event.map(Self::Vendor);
        }
        Event::try_from(packet).map(Self::Core)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aci::hal::HalEndOfRadioActivityEvent;
    use crate::wire::VendorEvent;

    #[test]
    fn vendor_events_the_target_emits_decode_as_aci_events() {
        let [low, high] = HalEndOfRadioActivityEvent::CODE.to_le_bytes();
        let params = [low, high, 1, 2, 0x10, 0x20, 0x30, 0x40, 3, 4];
        let packet = EventPacket {
            kind: EventKind::Vendor,
            data: &params,
        };
        assert!(matches!(
            BleEvent::from_packet(packet),
            Ok(BleEvent::Vendor(AciEvent::HalEndOfRadioActivity(_)))
        ));
        assert!(
            BleEvent::from_packet(EventPacket {
                kind: EventKind::Vendor,
                data: &params[..4],
            })
            .is_err(),
            "a known vendor event too short"
        );
    }

    /// A BIG_Handle is one byte, as bt-hci decodes it from 0.11.
    #[cfg(any(feature = "stack-wba-full", feature = "stack-wba-link-layer-only"))]
    #[test]
    fn big_events_decode_a_one_byte_handle() {
        use bt_hci::event::le::{LeBigSyncLost, LeEvent};
        use bt_hci::param::{BigHandle, Status};

        // LE BIG Sync Lost: subevent code, BIG_Handle, reason.
        let packet = EventPacket {
            kind: EventKind::Le,
            data: &[0x1E, 0x05, 0x13],
        };
        assert!(matches!(
            BleEvent::from_packet(packet),
            Ok(BleEvent::Core(Event::Le(LeEvent::LeBigSyncLost(
                LeBigSyncLost { big_handle, reason }
            )))) if big_handle == BigHandle(0x05) && reason == Status::new(0x13)
        ));
        let packet = EventPacket {
            kind: EventKind::Le,
            data: &[0x1E, 0x05],
        };
        assert!(BleEvent::from_packet(packet).is_err(), "too short");
    }

    #[test]
    fn other_events_decode_with_bt_hci() {
        let packet = EventPacket {
            kind: EventKind::Vendor,
            data: &[0xFE, 0xFF, 1],
        };
        assert!(matches!(
            BleEvent::from_packet(packet),
            Ok(BleEvent::Core(Event::Vendor(_)))
        ));
        // Disconnection Complete: status, handle, reason.
        let packet = EventPacket {
            kind: EventKind::DisconnectionComplete,
            data: &[0, 1, 0, 0x13],
        };
        assert!(matches!(
            BleEvent::from_packet(packet),
            Ok(BleEvent::Core(Event::DisconnectionComplete(_)))
        ));
    }
}
