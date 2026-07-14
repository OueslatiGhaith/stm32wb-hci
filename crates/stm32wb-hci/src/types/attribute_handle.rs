//! ATT attribute identifiers shared by commands and events.

/// Newtype for an ATT attribute handle.
///
/// Attribute handles are protocol identifiers, not general integers, and
/// should not be manipulated arithmetically.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeHandle(pub u16);
