//! Encryption key types used by STM32WB vendor commands.

use core::fmt::{Debug, Formatter, Result};

/// A 128-bit encryption key.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EncryptionKey(pub [u8; 16]);

impl Debug for EncryptionKey {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        write!(formatter, "AES-128 Key ({:X?})", self.0)
    }
}
