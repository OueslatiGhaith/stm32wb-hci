//! Standard LE commands supported by STM32WB but not yet provided by `bt-hci`.
//!
//! These command descriptors deliberately stay close to the Bluetooth HCI
//! wire definition. They are public raw command types: execute a synchronous
//! one with [`bt_hci::cmd::SyncCmd::exec`] and an asynchronous one with
//! [`bt_hci::cmd::AsyncCmd::exec`]. This complements the standard command
//! types re-exported as [`crate::bt_hci`].

use bt_hci::cmd::cmd;

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

#[cfg(since_fw_0_17_0)]
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

#[cfg(since_fw_0_23_0)]
cmd! {
    /// LE Set Resolvable Private Address Timeout v2 command.
    ///
    /// The parameters are the advertising handle, own-address type, and the
    /// two-byte timeout.
    LeSetResolvablePrivateAddressTimeoutV2(LE, 0x009E) {
        Params = [u8; 4];
        Return = ();
    }
}
