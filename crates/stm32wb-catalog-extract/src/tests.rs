//! C analysis tests over small fixtures written in the generated-code idiom.

use std::fs;
use std::sync::Mutex;

use clang::{Clang, Index};
use stm32wb_catalog::{CommandScope, Completion, Layout};

use crate::c::{self, Shim};
use crate::commands;

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
