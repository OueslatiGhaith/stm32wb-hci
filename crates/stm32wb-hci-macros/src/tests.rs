use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

fn parse(source: TokenStream2) -> VendorCommand {
    syn::parse2(source).unwrap()
}

fn parse_events(source: TokenStream2) -> VendorEvents {
    syn::parse2(source).unwrap()
}

fn parse_wire_type(source: TokenStream2) -> SemanticWireType {
    syn::parse2(source).unwrap()
}

#[test]
fn directly_generates_fixed_command_complete_unit_return() {
    let command = parse(quote! {
        GapSetIoCapability(cgid = 0x1, cid = 0x05) {
            Params = { io_capability: IoCapability => 1, };
            Completion = CommandComplete;
            Return = ();
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("pub struct GapSetIoCapability"));
    assert!(generated.contains("SyncCmd for GapSetIoCapability"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_fixed_command_status() {
    let command = parse(quote! {
        GapPeripheralSecurityRequest(cgid = 0x1, cid = 0x0D) {
            Params = { conn_handle: ConnHandle => 2, };
            Completion = CommandStatus;
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("AsyncCmd for GapPeripheralSecurityRequest"));
    assert!(!generated.contains("SyncCmd for GapPeripheralSecurityRequest"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_fixed_named_return() {
    let command = parse(quote! {
        CmdGapInit(cgid = 0x1, cid = 0x0A) {
            Params = {
                role: Role => 1,
                privacy_enabled: bool => 1,
                dev_name_characteristic_len: u8 => 1,
            };
            Completion = CommandComplete;
            Return = GapInit {
                service_handle: AttributeHandle => 2,
                dev_name_handle: AttributeHandle => 2,
                appearance_handle: AttributeHandle => 2,
            };
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("pub struct GapInit"));
    assert!(generated.contains("ReturnBuf = [u8 ; 6usize]"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_unit_params_and_default() {
    let command = parse(quote! {
        HalGetFirmwareRevision(cgid = 0x0, cid = 0x00) {
            Params = ();
            Completion = CommandComplete;
            Return = HalFirmwareRevision { revision: u16 => 2, };
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("Default for HalGetFirmwareRevision"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_fixed_constraints_and_try_new() {
    let command = parse(quote! {
        GapAdditionalBeaconStart(cgid = 0x1, cid = 0x30) {
            Params = {
                advertising_interval_min: u16 => 2,
                advertising_interval_max: u16 => 2,
                advertising_channel_map: AdvertisingChannelMap => 1,
            };
            Constraints = {
                range(advertising_interval_min, 0x0020, 0x4000);
                ordered(advertising_interval_min, advertising_interval_max);
                non_empty(advertising_channel_map);
            };
            Completion = CommandComplete;
            Return = ();
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("pub fn try_new"));
    assert!(generated.contains("HciConstraintError :: new"));
    assert!(generated.contains("advertising_channel_map . is_empty"));
    assert!(!generated.contains("pub fn new"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_counted_tagged_and_trailing_params() {
    let command = parse(quote! {
        VariableParams(cgid = 0x2, cid = 0x01) {
            Params<'a> = {
                bytes: &'a [u8] => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    max_len: 16,
                },
                uuid: &'a Uuid => {
                    kind: tagged,
                    tag: u8 => 1,
                    variants: {
                        Uuid::Uuid16(value) => {
                            tag: 0x01,
                            fields: { value: u16 => 2, },
                        },
                        Uuid::Uuid128(value) => {
                            tag: 0x02,
                            fields: { value: [u8; 16] => 16, },
                        },
                    },
                    min_len: 3,
                    max_len: 17,
                },
                tail: &'a [u8] => {
                    kind: trailing_bytes,
                    min_len: 0,
                    max_len: 8,
                },
            };
            Completion = CommandStatus;
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("CountedBytes"));
    assert!(generated.contains("TaggedField"));
    assert!(generated.contains("TrailingBytes"));
    assert!(generated.contains("pub fn try_new"));
    assert!(generated.contains("HciLengthError"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_counted_and_bitmap_items() {
    let command = parse(quote! {
        VariableItems(cgid = 0x1, cid = 0x51) {
            Params<'a> = {
                list: &'a [Peer] => {
                    kind: counted_items,
                    count: u8 => 1,
                    item: Peer => 7,
                    max_items: 3,
                },
                phys: Phys => 1,
                selected: &'a [PhyParams] => {
                    kind: bitmap_items,
                    bitmap: phys,
                    mask: 0x05,
                    item: PhyParams => 5,
                    max_items: 2,
                },
            };
            Completion = CommandStatus;
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("CountedItems"));
    assert!(generated.contains("BitmapItems"));
    assert!(generated.contains("try_new (selected , phys , 0x05)"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_all_owned_variable_return_decoders() {
    let command = parse(quote! {
        HalReadConfigData(cgid = 0x0, cid = 0x0D) {
            Params = { param: ConfigParameter => 1, };
            Completion = CommandComplete;
            Return = HalReadConfigDataReturn {
                bytes: BoundedBytes<16> => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    max_len: 16,
                },
                items: BoundedItems<Item, 4> => {
                    kind: counted_items,
                    count: u8 => 1,
                    item: Item => 2,
                    max_items: 4,
                },
                tail: BoundedBytes<8> => {
                    kind: trailing_bytes,
                    min_len: 0,
                    max_len: 8,
                },
            };
        }
    });
    let generated = expand_vendor_command(&command).to_string();
    assert!(generated.contains("decode_declarative_counted_bytes"));
    assert!(generated.contains("decode_declarative_counted_items"));
    assert!(generated.contains("decode_declarative_trailing_bytes"));
    assert!(generated.contains("ReturnBuf = [u8 ; 34usize]"));
    assert!(!generated.contains("vendor_cmd !"));
}

#[test]
fn directly_generates_event_enum_payloads_dispatch_and_cfg() {
    let events = parse_events(quote! {
        /// No payload.
        Unit(0x0001) { Payload = (); }
        #[cfg(since_fw_0_17_0)]
        Fixed(0x0002) {
            Payload = { value: u16 => 2, };
        }
    });
    let generated = expand_vendor_events(&events).to_string();
    assert!(generated.contains("pub enum VendorEvent"));
    assert!(generated.contains("EventsLost (EventFlags)"));
    assert!(generated.contains("pub struct Fixed"));
    assert!(generated.contains("0x0001 =>"));
    assert!(generated.contains("0x0002 =>"));
    assert!(generated.contains("decode_hci_event_field"));
    assert_eq!(generated.matches("cfg (since_fw_0_17_0)").count(), 3);
    assert!(!generated.contains("vendor_event !"));
}

#[test]
fn directly_generates_every_owned_variable_event_decoder() {
    let events = parse_events(quote! {
        Counted(0x0001) {
            Payload = {
                data: BoundedBytes<8> => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    max_len: 8,
                },
            };
        }
        Items(0x0002) {
            Payload = {
                values: BoundedItems<Item, 3> => {
                    kind: counted_items,
                    count: u8 => 1,
                    item: Item => 2,
                    max_items: 3,
                },
            };
        }
        Records(0x0003) {
            Payload = {
                value: Records => {
                    kind: length_prefixed_records,
                    record_len: u8 => 1,
                    length: u8 => 1,
                    min_record_len: 2,
                    max_len: 8,
                },
            };
        }
        Tagged(0x0004) {
            Payload = {
                value: Tagged => {
                    kind: tagged_items,
                    tag: u8 => 1,
                    length: u8 => 1,
                    variants: {
                        1 => { item: Short => 2, max_items: 4, },
                        2 => { item: Long => 4, max_items: 2, },
                    },
                    max_len: 8,
                },
            };
        }
        Trailing(0x0005) {
            Payload = {
                value: BoundedBytes<4> => {
                    kind: trailing_bytes,
                    min_len: 0,
                    max_len: 4,
                },
            };
        }
    });
    let generated = expand_vendor_events(&events).to_string();
    assert!(generated.contains("decode_hci_event_counted_bytes"));
    assert!(generated.contains("decode_hci_event_counted_items"));
    assert!(generated.contains("decode_hci_event_length_prefixed_records"));
    assert!(generated.contains("decode_hci_event_tagged_items_variant"));
    assert!(generated.contains("decode_hci_event_trailing_bytes"));
    assert!(generated.contains("__stm32wb_event_data"));
    assert!(!generated.contains("vendor_event !"));
}

#[test]
fn directly_generates_dual_command_and_event_scalar_adapters() {
    let declaration = parse_wire_type(quote! {
        adapters: [command, event];
        open_scalar pub struct ChannelIndex: u8 => 1;
    });
    let generated = expand_wire_type(&declaration).to_string();
    assert!(generated.contains("HciEncodeField < 1 > for ChannelIndex"));
    assert!(generated.contains("HciDecodeField < 1 > for ChannelIndex"));
    assert!(generated.contains("HciEventField < 1 > for ChannelIndex"));
    assert!(!generated.contains("hci_open_scalar"));
}

#[test]
fn directly_generates_cfg_aware_closed_enum_adapters() {
    let declaration = parse_wire_type(quote! {
        adapters: [command, event];
        closed
        pub enum State: u8 => 1 {
            Idle = 0,
            #[cfg(since_fw_0_24_0)]
            Active = 1,
        }
        TryFromError = BadState => BadState;
        EventError = Error::from;
    });
    let generated = expand_wire_type(&declaration).to_string();
    assert!(generated.contains("HciEncodeField < 1 > for State"));
    assert!(generated.contains("HciEventField < 1 > for State"));
    assert!(generated.contains("TryFrom < u8 > for State"));
    assert!(generated.matches("cfg (since_fw_0_24_0)").count() >= 4);
}

#[test]
fn directly_generates_both_composite_directions_from_one_shape() {
    let declaration = parse_wire_type(quote! {
        adapters: [command, event];
        composite Interval => 4 {
            Fields = {
                minimum: u16 => 2,
                maximum: u16 => 2,
            };
            Encode = |value| { (value.minimum, value.maximum) };
            Decode = { Ok(Self { minimum, maximum }) };
        }
    });
    let generated = expand_wire_type(&declaration).to_string();
    assert!(generated.contains("HciEncodeField < 4 > for Interval"));
    assert!(generated.contains("HciEventField < 4 > for Interval"));
    assert!(generated.contains("const _ : [() ; 4] = [() ; 0 + 2 + 2]"));
}
