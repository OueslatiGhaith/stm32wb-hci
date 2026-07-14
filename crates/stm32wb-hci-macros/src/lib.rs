//! Procedural entry points for the declarative STM32WB protocol catalog.

use proc_macro::TokenStream;
use stm32wb_hci_schema::{SemanticWireType, VendorCommand, VendorEvents};

mod vendor_command;
mod vendor_event;
mod wire_type;

use vendor_command::expand_vendor_command;
use vendor_event::expand_vendor_events;
use wire_type::expand_wire_type;

/// Declare one complete STM32WB vendor command.
///
/// The declaration is the source of truth for the command's vendor opcode,
/// request wire layout, completion mechanism, return wire layout, and
/// cross-field constraints. The same syntax is parsed by this proc macro and
/// by the compliance tool through `stm32wb-hci-schema`.
///
/// ```text
/// vendor_cmd! {
///     GapSetIoCapability(cgid = 0x1, cid = 0x05) {
///         Params = { io_capability: IoCapability => 1, };
///         Completion = CommandComplete;
///         Return = ();
///     }
/// }
/// ```
///
/// `cgid` is a three-bit command-group ID and `cid` is a seven-bit command ID.
/// The generated command derives its vendor OCF and HCI opcode from those two
/// values.
///
/// `Params` is either `()` or an inline field body. Fixed fields use
/// `field: Type => width`. Borrowing or variable fields use `Params<'a>` and
/// one of these typed schemas:
///
/// - `counted_bytes`: a count field followed by `min_len` through `max_len`
///   bytes; `min_len` defaults to zero.
/// - `counted_items`: a count field followed by `min_items` through
///   `max_items` fixed-width items; `min_items` defaults to zero.
/// - `tagged`: a fixed-width discriminator and one fixed-width variant body.
/// - `trailing_bytes`: a bounded field that consumes the remaining bytes.
/// - `bitmap_items`: fixed-width items selected by bits in an earlier field.
///
/// `CommandComplete` requires `Return = ();` or an inline named return type.
/// `CommandStatus` has no `Return` declaration and implements `AsyncCmd`.
/// Fixed, infallible commands expose `new`; constrained or variable commands
/// expose `try_new` with `HciConstraintError`, `HciLengthError`, or their
/// combined `HciValidationError` as appropriate. Variable construction checks
/// both each field's declared bound and the aggregate 255-byte HCI parameter
/// limit.
///
/// `Constraints` are evaluated in declaration order and stop at the first
/// failure. Supported relationships are `ordered`, `ordered_when_in_range`,
/// `range`, `one_of`, `one_of_or_range`, `paired_value`, `implies_eq`,
/// `implies_range`, `implies_one_of_or_range`, `implies_len_at_least`,
/// `implies_len_eq`, `len_eq`, `len_at_most`, `offset_len_at_most`, and
/// `non_empty`. Intrinsic
/// validity should remain in the semantic field type; constraints describe
/// relationships or command-specific subsets.
///
/// Selector-dependent checks use
/// `implies_*(selector, selected_value, dependent_field, ...)`. Length checks
/// call `.len()` on their field. Sparse domains use a nonempty `[value, ...]`
/// list followed, for `*_one_of_or_range`, by inclusive range endpoints.
#[proc_macro]
pub fn vendor_cmd(input: TokenStream) -> TokenStream {
    match syn::parse::<VendorCommand>(input) {
        Ok(command) => expand_vendor_command(&command).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Declare the complete STM32WB vendor-event catalog.
///
/// Each declaration owns its 16-bit vendor event code and complete payload
/// schema. Unit payloads generate unit `VendorEvent` variants; inline payloads
/// generate an owned public payload structure and a tuple variant carrying it.
/// Fixed fields use `field: Type => width`. Owned variable payload fields may
/// use `counted_bytes`, `counted_items`, `length_prefixed_records`,
/// `tagged_items`, or `trailing_bytes`.
///
/// The generated `VendorEvent::new` requires the two-byte event code, decodes
/// every declared field in order, and rejects both truncated and trailing
/// bytes. Event `cfg` attributes gate the enum variant, payload type, and
/// dispatch arm. Complementary `before_fw_*` and `since_fw_*` declarations may
/// therefore reuse a name or wire code when a firmware boundary changes its
/// shape.
#[proc_macro]
pub fn vendor_event(input: TokenStream) -> TokenStream {
    match syn::parse::<VendorEvents>(input) {
        Ok(events) => expand_vendor_events(&events).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Declare a semantic value and the protocol adapters for its canonical HCI
/// wire representation.
///
/// The shared parser validates adapter-specific requirements before expansion.
/// `command` generates `HciEncodeField` and, where meaningful,
/// `HciDecodeField`. `event` generates `HciEventField`, while `conversion` is
/// available for closed enums which need scalar conversions without a
/// fixed-width decoder. Multiple adapters derive from the same declaration.
///
/// Supported shapes are:
///
/// - `closed`: a closed scalar enum. Event and conversion adapters require
///   `TryFromError`; event decoding additionally requires `EventError`.
/// - `open_enum`: known variants plus `_ => Fallback`, preserving unknown raw
///   values.
/// - `open_scalar`: a transparent semantic newtype accepting every raw value.
/// - `ranged`: an inclusive scalar range with an optional named sentinel.
/// - `bitflags`: one bitflags declaration shared by ordinary and `defmt`
///   builds.
/// - `composite`: an ordered exact-width field decomposition with `Encode`,
///   `Decode`, or both according to its adapters.
/// - `primitive` and `transparent`: crate-internal adapter declarations for
///   built-in scalars and existing tuple newtypes.
///
/// Intrinsic domains belong here. Relationships between multiple command
/// parameters remain in `vendor_cmd!` constraints.
///
/// ```text
/// wire_type! {
///     adapters: [command, event];
///     ranged pub struct L2CocMtu: u16 => 2 {
///         minimum: 23,
///         maximum: u16::MAX,
///     }
///     EventError = map_l2cap_mtu_error;
/// }
/// ```
#[proc_macro]
pub fn wire_type(input: TokenStream) -> TokenStream {
    match syn::parse::<SemanticWireType>(input) {
        Ok(declaration) => expand_wire_type(&declaration).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[cfg(test)]
mod tests;
