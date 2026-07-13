//! Normalized declarative payload envelopes used at compliance boundaries.
//!
//! Every envelope describes only the bytes owned by the declaration being
//! checked: command parameters exclude the HCI command header, command
//! returns exclude Command Complete framing and its status byte, and vendor
//! event payloads exclude the two-byte vendor event code.

use std::fmt;

/// Inclusive byte-length bounds for one declarative wire payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WireEnvelope {
    pub(crate) minimum: usize,
    pub(crate) maximum: usize,
}

impl WireEnvelope {
    pub(crate) const fn fixed(length: usize) -> Self {
        Self {
            minimum: length,
            maximum: length,
        }
    }

    pub(crate) const fn bounded(minimum: usize, maximum: usize) -> Self {
        assert!(minimum <= maximum, "wire envelope minimum exceeds maximum");
        Self { minimum, maximum }
    }

    pub(crate) const fn is_fixed(self) -> bool {
        self.minimum == self.maximum
    }
}

impl fmt::Display for WireEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_fixed() {
            write!(formatter, "{} bytes", self.minimum)
        } else {
            write!(formatter, "{}..={} bytes", self.minimum, self.maximum)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_fixed_and_bounded_envelopes() {
        assert_eq!(WireEnvelope::fixed(3).to_string(), "3 bytes");
        assert_eq!(WireEnvelope::bounded(1, 7).to_string(), "1..=7 bytes");
    }
}
