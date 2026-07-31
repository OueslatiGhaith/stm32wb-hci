//! Standard LE commands supported by STM32WB but not yet provided by `bt-hci`.
//!
//! These command descriptors deliberately stay close to the Bluetooth HCI
//! wire definition. They are public raw command types: execute a synchronous
//! one with [`bt_hci::cmd::SyncCmd::exec`] and an asynchronous one with
//! [`bt_hci::cmd::AsyncCmd::exec`]. This complements the standard command
//! types re-exported as [`crate::bt_hci`].

use bt_hci::cmd::cmd;

/// Invalid minimum/maximum range for the controller's RPA rotation timeout.
#[cfg(since_fw_1_23_0)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ResolvablePrivateAddressTimeoutError {
    /// One endpoint is outside the controller's one-second to one-hour domain.
    OutOfRange(u16),
    /// The minimum timeout exceeds the maximum timeout.
    Inverted { minimum: u16, maximum: u16 },
}

/// Validated minimum and maximum RPA rotation timeouts, in seconds.
#[cfg(since_fw_1_23_0)]
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResolvablePrivateAddressTimeoutRange {
    minimum: u16,
    maximum: u16,
}

#[cfg(since_fw_1_23_0)]
impl ResolvablePrivateAddressTimeoutRange {
    /// Smallest timeout accepted by the controller, in seconds.
    pub const MINIMUM_SECONDS: u16 = 1;
    /// Largest timeout accepted by the controller, in seconds.
    pub const MAXIMUM_SECONDS: u16 = 0x0E10;

    /// Construct an ordered timeout range from values expressed in seconds.
    pub const fn try_new(
        minimum: u16,
        maximum: u16,
    ) -> Result<Self, ResolvablePrivateAddressTimeoutError> {
        if minimum < Self::MINIMUM_SECONDS || minimum > Self::MAXIMUM_SECONDS {
            return Err(ResolvablePrivateAddressTimeoutError::OutOfRange(minimum));
        }
        if maximum < Self::MINIMUM_SECONDS || maximum > Self::MAXIMUM_SECONDS {
            return Err(ResolvablePrivateAddressTimeoutError::OutOfRange(maximum));
        }
        if minimum > maximum {
            return Err(ResolvablePrivateAddressTimeoutError::Inverted { minimum, maximum });
        }
        Ok(Self { minimum, maximum })
    }

    /// Minimum timeout in seconds.
    pub const fn minimum_seconds(self) -> u16 {
        self.minimum
    }

    /// Maximum timeout in seconds.
    pub const fn maximum_seconds(self) -> u16 {
        self.maximum
    }
}

#[cfg(all(since_fw_1_23_0, feature = "defmt"))]
impl defmt::Format for ResolvablePrivateAddressTimeoutRange {
    fn format(&self, formatter: defmt::Formatter) {
        let minimum = self.minimum;
        let maximum = self.maximum;
        defmt::write!(formatter, "{}..={} s", minimum, maximum);
    }
}

#[cfg(since_fw_1_23_0)]
unsafe impl bt_hci::FixedSizeValue for ResolvablePrivateAddressTimeoutRange {
    fn is_valid(data: &[u8]) -> bool {
        if data.len() != 4 {
            return false;
        }
        let minimum = u16::from_le_bytes([data[0], data[1]]);
        let maximum = u16::from_le_bytes([data[2], data[3]]);
        Self::try_new(minimum, maximum).is_ok()
    }
}

cmd! {
    /// LE Receiver Test command.
    ///
    /// The parameter is the receive-channel index.
    LeReceiverTest(LE, 0x001D) {
        Params = [u8; 1];
        Return = ();
    }
}

cmd! {
    /// LE Transmitter Test command.
    ///
    /// Parameters are the transmit-channel index, test-data length, and
    /// payload pattern.
    LeTransmitterTest(LE, 0x001E) {
        Params = [u8; 3];
        Return = ();
    }
}

cmd! {
    /// LE Read Local P-256 Public Key command.
    ///
    /// Completion data is delivered by the LE Read Local P-256 Public Key
    /// Complete event rather than Command Complete.
    LeReadLocalP256PublicKey(LE, 0x0025) {
        Params = ();
    }
}

cmd! {
    /// LE Generate DHKey command.
    ///
    /// The 64-byte parameter is the remote P-256 public key. Completion data
    /// is delivered by the LE Generate DHKey Complete event.
    LeGenerateDhkey(LE, 0x0026) {
        Params = [u8; 64];
    }
}

cmd! {
    /// LE Read Peer Resolvable Address command.
    ///
    /// Its parameter is an identity-address type followed by a six-byte
    /// identity address; the return value is the six-byte resolvable address.
    LeReadPeerResolvableAddress(LE, 0x002B) {
        Params = [u8; 7];
        Return = [u8; 6];
    }
}

cmd! {
    /// LE Read Local Resolvable Address command.
    ///
    /// Its parameter is an identity-address type followed by a six-byte
    /// identity address; the return value is the six-byte resolvable address.
    LeReadLocalResolvableAddress(LE, 0x002C) {
        Params = [u8; 7];
        Return = [u8; 6];
    }
}

cmd! {
    /// LE Receiver Test v2 command.
    ///
    /// Parameters are receive frequency, PHY, and modulation index.
    LeReceiverTestV2(LE, 0x0033) {
        Params = [u8; 3];
        Return = ();
    }
}

cmd! {
    /// LE Transmitter Test v2 command.
    ///
    /// Parameters are transmit frequency, test-data length, payload pattern,
    /// and PHY.
    LeTransmitterTestV2(LE, 0x0034) {
        Params = [u8; 4];
        Return = ();
    }
}

#[cfg(since_fw_1_17_0)]
cmd! {
    /// LE Generate DHKey v2 command.
    ///
    /// The 65-byte parameter is the remote P-256 public key followed by the
    /// key type. Completion data is delivered by the LE Generate DHKey
    /// Complete event.
    LeGenerateDhkeyV2(LE, 0x005E) {
        Params = [u8; 65];
    }
}

#[cfg(since_fw_1_23_0)]
cmd! {
    /// LE Set Resolvable Private Address Timeout v2 command.
    ///
    /// The parameter sets an ordered range from which the controller randomly
    /// chooses each new RPA timeout.
    LeSetResolvablePrivateAddressTimeoutV2(LE, 0x009E) {
        Params = ResolvablePrivateAddressTimeoutRange;
        Return = ();
    }
}
