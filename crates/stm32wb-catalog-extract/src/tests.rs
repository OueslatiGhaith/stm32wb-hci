//! C analysis tests over small fixtures written in the generated-code idiom.

use std::collections::BTreeMap;
use std::fs;
use std::sync::Mutex;

use clang::{Clang, Index};
use stm32wb_catalog::{CommandScope, Completion, EventScope, Layout};

use crate::c::{self, Shim};
use crate::{commands, events, shci};

/// libclang allows one `Clang` instance per process at a time.
static CLANG: Mutex<()> = Mutex::new(());

const PRELUDE: &str = r#"
#include <stdint.h>
#define BLE_CMD_MAX_PARAM_LEN 255
#define BLE_EVT_MAX_PARAM_LEN 255
#define __PACKED_STRUCT struct __attribute__((packed))
#define __PACKED_UNION union __attribute__((packed))
typedef uint8_t tBleStatus;
struct hci_request { uint16_t ogf; uint16_t ocf; int event; void *cparam; int clen; void *rparam; int rlen; };
int hci_send_req(struct hci_request *r, uint8_t async);
void *Osal_MemCpy(void *d, const void *s, unsigned int n);
void *Osal_MemSet(void *p, int v, unsigned int n);
"#;

/// Parse a fixture and run `analyze` on it with a fresh libclang instance.
fn with_fixture<T>(source: &str, analyze: impl FnOnce(&clang::TranslationUnit<'_>) -> T) -> T {
    let _guard = CLANG
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let clang = Clang::new().expect("libclang is available");
    let index = Index::new(&clang, false, false);
    let shim = Shim::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.c");
    fs::write(&path, format!("{PRELUDE}{source}")).unwrap();
    let unit = c::parse(&index, &path, &shim, &[]).unwrap();
    analyze(&unit)
}

fn command_fixture(source: &str) -> Result<Vec<commands::ExtractedCommand>, String> {
    with_fixture(source, |unit| {
        commands::extract(unit, &c::records(unit), CommandScope::Vendor)
    })
}

fn fields(layout: &Layout) -> Vec<String> {
    match layout {
        Layout::Fields(fields) => fields.iter().map(ToString::to_string).collect(),
        Layout::Unresolved(reason) => panic!("unresolved: {reason}"),
    }
}

const SET_NAME: &str = r#"
typedef __PACKED_STRUCT { uint8_t Mode; uint8_t Name_Length; uint8_t Name[BLE_CMD_MAX_PARAM_LEN - 2]; } aci_set_name_cp0;
typedef __PACKED_STRUCT { uint16_t Interval; } aci_set_name_cp1;
tBleStatus aci_set_name(uint8_t Mode, uint8_t Name_Length, const uint8_t *Name, uint16_t Interval)
{
  struct hci_request rq;
  uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
  aci_set_name_cp0 *cp0 = (aci_set_name_cp0*)(cmd_buffer);
  aci_set_name_cp1 *cp1 = (aci_set_name_cp1*)(cmd_buffer + 2 + Name_Length * (sizeof(uint8_t)));
  tBleStatus status = 0;
  int index_input = 0;
  cp0->Mode = Mode;
  index_input += 1;
  cp0->Name_Length = Name_Length;
  index_input += 1;
  {
    Osal_MemCpy((void*)&cp0->Name, (const void*)Name, Name_Length);
    index_input += Name_Length;
    {
      cp1->Interval = Interval;
    }
    index_input += 2;
  }
  Osal_MemSet(&rq, 0, sizeof(rq));
  rq.ogf = 0x3f;
  rq.ocf = 0x083;
  rq.event = 0x0F;
  rq.cparam = cmd_buffer;
  rq.clen = index_input;
  rq.rparam = &status;
  rq.rlen = 1;
  if (hci_send_req(&rq, 0) < 0)
    return 0xFF;
  return status;
}
"#;

#[test]
fn commands_follow_buffer_writes_and_counts() {
    let commands = command_fixture(SET_NAME).unwrap();
    let [command] = &commands[..] else {
        panic!("one command expected");
    };
    assert_eq!(command.opcode, 0xFC83);
    assert_eq!(command.completion, Completion::CommandStatus);
    assert_eq!(command.returns, None);
    assert_eq!(
        fields(&command.params),
        [
            "Mode: u8",
            "Name_Length: u8",
            "Name: [u8; Name_Length] (capacity 253)",
            "Interval: u16",
        ]
    );
    assert_eq!(
        command.proven_counts,
        [("Name_Length".to_owned(), "Name".to_owned(), true)]
    );
}

#[test]
fn mismatched_size_increments_leave_the_layout_unresolved() {
    let wrong = SET_NAME.replace(
        "cp0->Mode = Mode;\n  index_input += 1;",
        "cp0->Mode = Mode;\n  index_input += 2;",
    );
    let commands = command_fixture(&wrong).unwrap();
    assert!(matches!(&commands[0].params, Layout::Unresolved(reason) if reason.contains("Mode")));

    let reordered = SET_NAME.replace(
        "cp0->Mode = Mode;\n  index_input += 1;\n  cp0->Name_Length = Name_Length;\n  index_input += 1;",
        "cp0->Name_Length = Name_Length;\n  index_input += 1;\n  cp0->Mode = Mode;\n  index_input += 1;",
    );
    let commands = command_fixture(&reordered).unwrap();
    assert!(matches!(&commands[0].params, Layout::Unresolved(reason) if reason.contains("order")));
}

#[test]
fn identity_failures_are_fatal() {
    let error = command_fixture(&SET_NAME.replace("rq.ocf = 0x083;", "")).unwrap_err();
    assert!(error.contains("rq.ocf"), "{error}");
    let error =
        command_fixture(&SET_NAME.replace("rq.event = 0x0F;", "rq.event = 0x3E;")).unwrap_err();
    assert!(error.contains("rq.event"), "{error}");
    let error = command_fixture(&SET_NAME.replace("rq.ogf = 0x3f;", "rq.ogf = 0x08;")).unwrap_err();
    assert!(error.contains("OGF"), "{error}");
}

#[test]
fn unions_follow_switches_and_conditionals() {
    let source = r#"
typedef __PACKED_UNION { uint16_t Uuid_16; uint8_t Uuid_128[16]; } Uuid_t;
typedef __PACKED_STRUCT { uint8_t Uuid_Type; Uuid_t Uuid; } aci_add_cp0;
typedef __PACKED_STRUCT { uint8_t Records; } aci_add_cp1;
typedef __PACKED_STRUCT { uint8_t Status; uint16_t Handle; } aci_add_rp0;
tBleStatus aci_add(uint8_t Uuid_Type, const Uuid_t *Uuid, uint8_t Records, uint16_t *Handle)
{
  struct hci_request rq;
  uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
  aci_add_cp0 *cp0 = (aci_add_cp0*)(cmd_buffer);
  aci_add_cp1 *cp1 = (aci_add_cp1*)(cmd_buffer + 1 + (Uuid_Type == 1 ? 2 : (Uuid_Type == 2 ? 16 : 0)));
  aci_add_rp0 resp;
  int index_input = 0;
  cp0->Uuid_Type = Uuid_Type;
  index_input += 1;
  {
    uint8_t size;
    switch (Uuid_Type)
    {
      case 1: size = 2; break;
      case 2: size = 16; break;
      default: return 0x47;
    }
    Osal_MemCpy((void*)&cp0->Uuid, (const void*)Uuid, size);
    index_input += size;
    {
      cp1->Records = Records;
    }
    index_input += 1;
  }
  rq.ogf = 0x3f;
  rq.ocf = 0x102;
  rq.rparam = &resp;
  rq.rlen = sizeof(resp);
  if (hci_send_req(&rq, 0) < 0)
    return 0xFF;
  *Handle = resp.Handle;
  return 0;
}

typedef __PACKED_STRUCT { uint8_t Include_Type; Uuid_t Include; } aci_include_cp0;
tBleStatus aci_include(uint8_t Include_Type, const Uuid_t *Include)
{
  struct hci_request rq;
  uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
  aci_include_cp0 *cp0 = (aci_include_cp0*)(cmd_buffer);
  tBleStatus status = 0;
  int index_input = 0;
  int uuid_size = (Include_Type == 2) ? 16 : 2;
  cp0->Include_Type = Include_Type;
  index_input += 1;
  Osal_MemCpy((void*)&cp0->Include, (const void*)Include, uuid_size);
  index_input += uuid_size;
  rq.ogf = 0x3f;
  rq.ocf = 0x103;
  rq.rparam = &status;
  rq.rlen = 1;
  if (hci_send_req(&rq, 0) < 0)
    return 0xFF;
  return status;
}
"#;
    let commands = command_fixture(source).unwrap();
    assert_eq!(
        fields(&commands[0].params),
        [
            "Uuid_Type: u8",
            "Uuid: union(Uuid_Type) { 1 => 2, 2 => 16 }",
            "Records: u8",
        ]
    );
    assert_eq!(
        fields(commands[0].returns.as_ref().unwrap()),
        ["Status: u8", "Handle: u16"]
    );
    assert_eq!(
        fields(&commands[1].params),
        [
            "Include_Type: u8",
            "Include: union(Include_Type) { 2 => 16, _ => 2 }"
        ]
    );
    assert_eq!(
        fields(commands[1].returns.as_ref().unwrap()),
        ["Status: u8"]
    );
}

#[test]
fn returns_trace_counts_through_output_parameters() {
    let source = r#"
typedef __PACKED_STRUCT { uint8_t Handle; uint16_t Duration; } Entry_t;
typedef __PACKED_STRUCT { uint8_t Offset; } aci_read_cp0;
typedef __PACKED_STRUCT { uint8_t Status; uint8_t Data_Length; uint8_t Data[(BLE_EVT_MAX_PARAM_LEN - 3) - 2]; } aci_read_rp0;
tBleStatus aci_read(uint8_t Offset, uint8_t *Data_Length, uint8_t *Data)
{
  struct hci_request rq;
  uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
  aci_read_cp0 *cp0 = (aci_read_cp0*)(cmd_buffer);
  aci_read_rp0 resp;
  int index_input = 0;
  cp0->Offset = Offset;
  index_input += 1;
  rq.ogf = 0x3f;
  rq.ocf = 0x00d;
  rq.rparam = &resp;
  rq.rlen = sizeof(resp);
  if (hci_send_req(&rq, 0) < 0)
    return 0xFF;
  if (resp.Status)
    return resp.Status;
  *Data_Length = resp.Data_Length;
  Osal_MemCpy((void*)Data, (const void*)resp.Data, *Data_Length);
  return 0;
}

typedef __PACKED_STRUCT { uint8_t Count; Entry_t Entry[(BLE_CMD_MAX_PARAM_LEN - 1)/sizeof(Entry_t)]; } aci_sets_cp0;
tBleStatus aci_sets(uint8_t Count, const Entry_t *Entry)
{
  struct hci_request rq;
  uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
  aci_sets_cp0 *cp0 = (aci_sets_cp0*)(cmd_buffer);
  tBleStatus status = 0;
  int index_input = 0;
  cp0->Count = Count;
  index_input += 1;
  Osal_MemCpy((void*)&cp0->Entry, (const void*)Entry, Count * (sizeof(Entry_t)));
  index_input += Count * (sizeof(Entry_t));
  rq.ogf = 0x3f;
  rq.ocf = 0x00e;
  rq.rparam = &status;
  rq.rlen = 1;
  if (hci_send_req(&rq, 0) < 0)
    return 0xFF;
  return status;
}
"#;
    let commands = command_fixture(source).unwrap();
    assert_eq!(
        fields(commands[0].returns.as_ref().unwrap()),
        [
            "Status: u8",
            "Data_Length: u8",
            "Data: [u8; Data_Length] (capacity 250)"
        ]
    );
    assert_eq!(
        fields(&commands[1].params),
        ["Count: u8", "Entry: [Entry_t; Count] (capacity 84)"]
    );
    assert_eq!(commands[1].structs.keys().collect::<Vec<_>>(), ["Entry_t"]);
}

#[test]
fn events_are_read_through_trivial_process_functions() {
    let source = r#"
typedef struct { uint16_t evt_code; void (*process)(const uint8_t *in); } hci_event_table_t;
typedef __PACKED_STRUCT { uint16_t Connection_Handle; uint16_t Data_Length; uint8_t Data[(BLE_EVT_MAX_PARAM_LEN - 2) - 4]; } aci_modified_event_rp0;
typedef __PACKED_STRUCT { uint8_t Kind; const uint8_t *Data; } Report_t;
typedef __PACKED_STRUCT { uint8_t Num; Report_t Report[1]; } hci_report_event_rp0;
void aci_modified_event(uint16_t Connection_Handle, uint16_t Data_Length, const uint8_t *Data);
void aci_ready_event(void);
void hci_report_event(uint8_t Num, const Report_t *Report);
static void aci_modified_event_process(const uint8_t *in);
static void aci_ready_event_process(const uint8_t *in);
static void hci_report_event_process(const uint8_t *in);
const hci_event_table_t hci_event_table[1] = { { 0x0005U, hci_report_event_process } };
const hci_event_table_t hci_le_event_table[0] = { };
const hci_event_table_t hci_vs_event_table[2] =
{
  { 0x0C01U, aci_modified_event_process },
  { 0x0400U, aci_ready_event_process },
};
static void aci_modified_event_process(const uint8_t *in)
{
  aci_modified_event_rp0 *rp0 = (void*)in;
  aci_modified_event(rp0->Connection_Handle, rp0->Data_Length, rp0->Data);
}
static void aci_ready_event_process(const uint8_t *in)
{
  aci_ready_event();
}
static void hci_report_event_process(const uint8_t *in)
{
  hci_report_event_rp0 *rp0 = (void*)in;
  Report_t Report[1];
  int i;
  for (i = 0; i < rp0->Num; i++) { hci_report_event(1, Report); }
}
"#;
    let events = with_fixture(source, |unit| events::extract(unit, &c::records(unit))).unwrap();
    let by_code = events
        .iter()
        .map(|event| ((event.scope, event.code), event))
        .collect::<BTreeMap<_, _>>();
    let modified = by_code[&(EventScope::Vendor, 0x0C01)];
    assert_eq!(modified.name, "aci_modified_event");
    assert_eq!(
        fields(&modified.payload),
        [
            "Connection_Handle: u16",
            "Data_Length: u16",
            "Data: [u8; Data_Length] (capacity 249)",
        ]
    );
    assert!(fields(&by_code[&(EventScope::Vendor, 0x0400)].payload).is_empty());
    assert!(matches!(
        &by_code[&(EventScope::Standard, 0x0005)].payload,
        Layout::Unresolved(reason) if reason.contains("procedurally")
    ));
}

#[test]
fn shci_payloads_require_adjacent_documentation() {
    let source = r#"
typedef enum { READY = 0x9200, END_WRITE, END_ERASE, UPDATE } SHCI_SUB_EVT_CODE_t;
typedef enum { RUNNING = 0 } Kind_t;
/**
 * READY
 * The coprocessor is ready.
 */
typedef __PACKED_STRUCT { Kind_t kind; } Ready_t;
/**
 * UPDATE
 * A section was updated.
 */
typedef __PACKED_STRUCT { uint32_t StartAddress; uint32_t Size; } Update_t;
/**
 * END_WRITE
 * Writing finished.
 */
/**
 * END_ERASE
 * Erasing finished.
 */

/* SYSTEM COMMAND */
typedef __PACKED_STRUCT { uint32_t MetaData[3]; } Header_t;
"#;
    let events = with_fixture(source, |unit| shci::extract(unit, &c::records(unit))).unwrap();
    let payloads = events
        .iter()
        .map(|event| (event.name.as_str(), &event.payload))
        .collect::<BTreeMap<_, _>>();
    assert!(matches!(payloads["READY"], Layout::Unresolved(reason) if reason.contains("enum")));
    assert_eq!(
        fields(payloads["UPDATE"]),
        ["StartAddress: u32", "Size: u32"]
    );
    assert!(matches!(payloads["END_WRITE"], Layout::Unresolved(_)));
    // END_ERASE's comment is separated from Header_t and must not document it.
    assert!(matches!(payloads["END_ERASE"], Layout::Unresolved(_)));
    assert_eq!(
        events
            .iter()
            .find(|event| event.name == "UPDATE")
            .unwrap()
            .code,
        0x9203
    );
}

#[test]
fn shci_commands_follow_their_wrappers() {
    let source = r#"
typedef __PACKED_STRUCT { uint8_t evtcode; uint8_t plen; uint8_t numcmd; uint16_t cmdcode; uint8_t payload[8]; } TL_CcEvt_t;
typedef struct { struct { struct { uint8_t payload[16]; } evt; } evtserial; } TL_EvtPacket_t;
void shci_send(uint16_t opcode, uint8_t length, uint8_t *parameters, TL_EvtPacket_t *response);
void *memcpy(void *destination, const void *source, unsigned int length);
#define OPCODE(ocf) ((0x3F << 10) + (ocf))
#define RESULT(index) (((TL_CcEvt_t *)(response->evtserial.evt.payload))->payload[index])
typedef __PACKED_STRUCT { uint8_t *Buffer; uint16_t Size; } Init_Param_t;
typedef __PACKED_STRUCT { uint32_t port; uint8_t pin; } Pa_Param_t;
typedef __PACKED_STRUCT { uint32_t relative_time; } Time_Param_t;

uint8_t GetState(uint8_t *p_error_code) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    shci_send(OPCODE(0x52), 0, 0, response);
    if (p_error_code != 0) { *p_error_code = (uint8_t)RESULT(1); }
    return RESULT(0);
}
uint8_t Init(Init_Param_t *pParam) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    shci_send(OPCODE(0x66), sizeof(Init_Param_t), (uint8_t *)pParam, response);
    return RESULT(0);
}
uint8_t Load(uint8_t key_index, uint8_t flag) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    buffer[0] = key_index;
    buffer[1] = (uint8_t)flag;
    shci_send(OPCODE(0x59), 2, buffer, response);
    return RESULT(0);
}
uint8_t Pa(uint32_t port, uint8_t pin) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    ((Pa_Param_t *)buffer)->port = port;
    ((Pa_Param_t *)buffer)->pin = pin;
    shci_send(OPCODE(0x72), 5, buffer, response);
    return RESULT(0);
}
uint8_t Time(Time_Param_t *pParam) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    shci_send(OPCODE(0x76), 0, 0, response);
    memcpy(&pParam->relative_time, &RESULT(1), sizeof(pParam->relative_time));
    return RESULT(0);
}
uint8_t Upgrade(uint32_t address) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    uint8_t length = 0;
    if (address != 0) { *(uint32_t *)buffer = address; length = 4; }
    shci_send(OPCODE(0x54), length, buffer, response);
    return RESULT(0);
}
uint8_t Gap(uint8_t value) {
    uint8_t buffer[16]; TL_EvtPacket_t *response = (TL_EvtPacket_t *)buffer;
    buffer[1] = value;
    shci_send(OPCODE(0x60), 2, buffer, response);
    return RESULT(0);
}
uint32_t LocalInformation(void) { return 0; }
"#;
    let commands = with_fixture(source, |unit| shci::commands(unit, &c::records(unit))).unwrap();
    let by_name = commands
        .iter()
        .map(|command| (command.name.as_str(), command))
        .collect::<BTreeMap<_, _>>();
    assert!(!by_name.contains_key("LocalInformation"));
    assert!(
        commands
            .iter()
            .all(|command| command.scope == CommandScope::System
                && command.completion == Completion::CommandComplete)
    );
    assert_eq!(by_name["GetState"].opcode, 0xFC52);
    assert!(fields(&by_name["GetState"].params).is_empty());
    assert_eq!(
        fields(by_name["GetState"].returns.as_ref().unwrap()),
        ["Status: u8", "error_code: u8"]
    );
    // Pointers in parameter structures are 32-bit addresses.
    assert_eq!(
        fields(&by_name["Init"].params),
        ["Buffer: u32", "Size: u16"]
    );
    assert_eq!(
        fields(&by_name["Load"].params),
        ["key_index: u8", "flag: u8"]
    );
    assert_eq!(fields(&by_name["Pa"].params), ["port: u32", "pin: u8"]);
    assert_eq!(
        fields(by_name["Time"].returns.as_ref().unwrap()),
        ["Status: u8", "relative_time: u32"]
    );
    assert!(matches!(
        &by_name["Upgrade"].params,
        Layout::Unresolved(reason) if reason.contains("run time")
    ));
    assert!(matches!(
        &by_name["Gap"].params,
        Layout::Unresolved(reason) if reason.contains("written at [1]")
    ));
}

/// The generated headers document a parameter addressing an ATT bearer with
/// both of its ranges.
const BEARER_DOC: &str = r#"
/**
 * @brief ACI_SET_NAME
 * @param Mode Mode.
 * @param Name_Length Length of Name.
 * @param Name Name.
 * @param Interval Specifies the ATT bearer for which the command
 *        applies.
 *        Values:
 *        - 0x0000 ... 0x0EFF: Unenhanced ATT bearer (the parameter is the
 *          connection handle)
 *        - 0xEA00 ... 0xEA3F: Enhanced ATT bearer (the LSB-byte of the
 *          parameter is the connection-oriented channel index)
 * @return Value indicating success or error code.
 */
tBleStatus aci_set_name(uint8_t Mode, uint8_t Name_Length, const uint8_t *Name, uint16_t Interval);
"#;

#[test]
fn bearers_come_from_the_header_documentation() {
    let commands = command_fixture(&format!("{BEARER_DOC}{SET_NAME}")).unwrap();
    let bearers = commands[0]
        .bearers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(bearers, ["Interval: 0xEA00..=0xEA3F"]);

    let commands = command_fixture(SET_NAME).unwrap();
    assert!(commands[0].bearers.is_empty());
}

#[test]
fn bearer_ranges_are_read_not_guessed() {
    let bearers = c::bearers_in(
        "/** @param Connection_Handle Values:\n *  - 0x0000 ... 0x0EFF: connection\n \
         *  - 0xEA00 ... 0xEA1F: Enhanced ATT bearer\n * @param Offset 0xEA00 is not in it\n \
         * @return Status */",
    );
    assert!(
        bearers
            .as_ref()
            .unwrap_err()
            .contains("Offset documents a value at 0xEA that is not a range"),
        "{bearers:?}"
    );

    let bearers = c::bearers_in(
        "/** @param Connection_Handle Values:\n *  - 0xEA00 ... 0xEA1F: Enhanced ATT bearer\n \
         * @return 0xEA00 is not a parameter */",
    )
    .unwrap();
    assert_eq!(bearers[0].to_string(), "Connection_Handle: 0xEA00..=0xEA1F");

    let error =
        c::bearers_in("/** @param Handle 0xEA00 ... 0xEA1F or 0xEA40 ... 0xEA7F */").unwrap_err();
    assert!(
        error.contains("several enhanced ATT bearer ranges"),
        "{error}"
    );
}

#[test]
fn domains_come_from_the_header_documentation() {
    let commands = command_fixture(&format!("{BEARER_DOC}{SET_NAME}")).unwrap();
    let (member, returned, domain) = &commands[0].domains[0];
    assert_eq!((member.as_str(), *returned), ("Interval", false));
    let items = domain
        .items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        items,
        [
            "0x0000..=0x0EFF: Unenhanced ATT bearer (the parameter is the connection handle)",
            "0xEA00..=0xEA3F: Enhanced ATT bearer (the LSB-byte of the parameter is the \
             connection-oriented channel index)",
        ]
    );
}

/// Written as the generated headers write them, from `aci_gap_set_discoverable`,
/// `aci_gap_init`, and `hci_read_transmit_power_level`.
const DOMAIN_DOC: &str = "/**
 * @brief Documented values.
 *
 * @param Advertising_Interval_Min Minimum advertising interval.
 *        Time = N * 0.625 ms.
 *        Values:
 *        - 0x0020 (20.000 ms)  ... 0x4000 (10240.000 ms)
 * @param Conn_Interval_Min Connection interval minimum value suggested by
 *        Peripheral.
 *        Values:
 *        - 0x0000 (NaN)
 *        - 0xFFFF (NaN) : No specific minimum
 *        - 0x0006 (7.50 ms)  ... 0x0C80 (4000.00 ms)
 * @param Role Bitmap of allowed roles.
 *        Flags:
 *        - 0x01: Peripheral
 *        - 0x02: Broadcaster
 * @param device_name_char_len Length of the device name characteristic
 * @param[out] Transmit_Power_Level Size: 1 Octet (signed integer)
 *        Units: dBm
 *        Values:
 *        - -30 ... 20
 * @return Value indicating success or error code.
 */";

#[test]
fn domains_read_values_flags_units_and_notes() {
    let domains = crate::domains::domains_in(DOMAIN_DOC).unwrap();
    let summary = domains
        .iter()
        .map(|(member, returned, domain)| {
            let items = domain
                .items
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{member}{} {:?} [{items}] {:?}",
                if *returned { " (out)" } else { "" },
                domain.kind,
                domain.unit_us
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        [
            "Advertising_Interval_Min Values [0x0020..=0x4000] Some(625)",
            "Conn_Interval_Min Values [0x0000: NaN, 0x0006..=0x0C80, 0xFFFF: No specific \
             minimum] Some(1250)",
            "Role Flags [0x01: Peripheral, 0x02: Broadcaster] None",
            "Transmit_Power_Level (out) Values [-30..=20] None",
        ]
    );
}

#[test]
fn domains_are_read_not_guessed() {
    let error = |comment: &str| crate::domains::domains_in(comment).unwrap_err();
    let unit = error(
        "/**\n * @param Interval Values:\n *        Values:\n *        \
         - 0x0020 (20.000 ms)  ... 0x4000 (10000.000 ms)\n */",
    );
    assert!(unit.contains("is written as 20.000 ms"), "{unit}");
    let stray = error(
        "/**\n * @param Mode Values\n *        Values:\n *        - 0x00: Off\n *     \
         Note: in the middle of the list\n */",
    );
    assert!(stray.contains("not an item"), "{stray}");
    let twice = error(
        "/**\n * @param Mode Mode\n *        Values:\n *        - 0x00: Off\n *        \
         Flags:\n *        - 0x01: On\n */",
    );
    assert!(twice.contains("two lists"), "{twice}");
    let value = error("/**\n * @param Mode Mode\n *        Values:\n *        - zero: Off\n */");
    assert!(value.contains("is not a value"), "{value}");
}
