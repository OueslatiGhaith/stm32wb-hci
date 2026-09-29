# STM32WB and STM32WBA wireless-interface catalogs

`stm32wb.toml` describes, for every supported STM32CubeWB release, what the
CPU2 wireless binaries accept and emit. It is the single source of protocol
facts for this repository. It and `annotations.toml` are embedded in the
`stm32wb-catalog` crate, so its dependents are rebuilt whenever the extractor
rewrites them.

`stm32wba.toml` describes, in the same format, the commands and events of the
BLE stack library of each supported STM32CubeWBA release; see
[STM32WBA](#stm32wba). It and `stm32wba-annotations.toml` are embedded the
same way.

## Layers

1. **Generated C.** Opcodes, completion kinds, and wire layouts are read from
   the tagged `ble_*_aci.c`, `ble_hci_le.c`, `ble_events.c`, `ble_types.h`,
   `shci.h`, and `shci.c` with libclang, and the status codes from
   `ble_defs.h`. Relationships (which member counts a buffer,
   which member selects a union alternative) come from symbol references in
   the wrapper code, not from member names. Event payloads have no such code:
   their buffers are counted by the member right before them, a rule of ST's
   generator that every release's command code is checked against. Each
   command layout the code proves must also follow from its `_cpN` and `_rp0`
   structures alone, by the same rule, or be reported as not doing so.
2. **ST documents.** Stack-profile availability comes from
   `STM32WB_BLE_Wireless_Interface.html`, whose "Events generated" lists
   must name the completion event every wrapper waits for; which binaries
   exist for each MCU family comes from each family's `Release_Notes.html`,
   whose every binary row must name a known stack profile.
3. **Annotations.** `annotations.toml` holds curated facts no ST artifact
   states in a machine-readable form. Each cites its source, and the audit
   rejects annotations that are dangling, stale, overlapping, or that
   contradict an extracted fact without `override = true`.

Layers 1 and 2 are generated and must never be edited by hand. A layout the
extractor cannot derive without guessing is written as
`{ unresolved = "reason" }`; an annotation may then supply it.

## Field notation

```text
Advertising_Type: u8                                        integer
Direct_Address: [u8; 6]                                     fixed array
Local_Name: [u8; Local_Name_Length] (capacity 242)          counted by an earlier member
Adv_Set: [Adv_Set_t; Number_of_Sets] (capacity 84)          counted structures
Service_UUID: union(Service_UUID_Type) { 1 => 2, 2 => 16 }  width selected by an earlier member
```

Optional members, written `fw_src_add: u32 (optional)`, end a layout: each is
sent only if every earlier one is.

## Documented values

Each command and event lists, under `domains`, the values its members may
take, read from the `Values:` and `Flags:` lists of the generated headers'
`@param` blocks. Each list has its own release range, so relabelling a value
does not split a definition:

```toml
[[commands.domains]]
releases = "1.15.0..=1.24.0"
member = "Advertising_Interval_Min"
values = ["0x0020..=0x4000"]
unit_us = 625
```

An item is a value or an inclusive range, written as the header writes it,
with its label or note. `flags` lists single bits and, where documented, a
named zero. `unit_us` is the duration of one unit, derived from the
durations the list writes next to its values and checked against every one
of them. `returned = true` marks a return parameter. Items may overlap, since
some lists depend on the stack profile or the MCU (`for BO variant`,
`not supported on STM32WB`); their labels say so. A byte array of up to 8
bytes, such as an event mask, takes the values of the little-endian integer
it holds; any other array of integers takes those of each element.

The fields of the structures commands and events carry list their values in
`ble_types.h`, in the same notation. `struct_domains` records them by
structure, over the releases documenting them:

```toml
[[struct_domains]]
releases = "1.15.0..=1.24.0"
structure = "Adv_Set_t"
member = "Duration"
values = ["0x0000: No advertising duration.", "0x0001..=0xFFFF: Advertising duration"]
unit_us = 10000
```

Every list the headers write is either recorded or reported by `extract`
and `check`, as for a list whose member's layout, or whose structure's, is
unresolved. A list header anywhere else than its place is an error.

Command returns include the leading status byte. Capacities are the element
capacities the generated C buffers declare.

System (SHCI) commands come from the `shci.c` wrappers: the parameters are
whatever each wrapper passes to `shci_send`, and the returns are the bytes of
the Command Complete payload it reads. A pointer in a system parameter
structure is an address the wireless CPU reads, recorded as a `u32`: the
wrappers are compiled for the Cortex-M4, whose AAPCS32 pointers are 4 bytes.

## Status codes

`statuses` lists the status codes the BLE stack returns besides the
Bluetooth Core's, read from the `BLE_STATUS_*` definitions of
`ble_defs.h`, each over the releases defining it with that value:

```toml
[[statuses]]
releases = "1.15.0..=1.24.0"
name = "BLE_STATUS_FAILED"
value = "0x91"
```

## STM32WBA

STM32CubeWBA ships its BLE stack as a prebuilt library: the generated headers
declare the command functions, the event callbacks, and the packed structures,
but no C fills or reads them. So its catalog takes:

- commands, events, opcodes, codes, and profile availability (columns BP, BF,
  PO, and LO; the full stack supports everything) from
  `STM32WBA_BLE_Wireless_Interface.html`;
- each command's completion from the "Events generated" list of its section,
  a rule checked against the code of every STM32WB release; a command whose
  section states none is left out and reported;
- layouts from the `_cpN` and `_rp0` structures of `ble_types.h` alone, by
  the rule every STM32WB layout the code proves is checked against. A union
  has no selector without code, so a layout holding one is unresolved. An
  event payload must be the members its `ble_events.h` callback receives, in
  order;
- values, bearers, and statuses as on STM32WB. A value labelled for one
  platform, such as `(only for STM32WB)` or `[not supported on STM32WBA]`,
  applies only to that platform's profiles.

A `Flags:` list whose items are not single bits (some CS events pack two
4-bit fields under one) is dropped and reported, as are the bearers of an
unresolved layout. The releases from v1.9.0 on pin
`Middlewares/ST/STM32_WPAN` as a submodule, read at its pinned commit, so the
clone needs `git submodule update --init Middlewares/ST/STM32_WPAN`.

A release is selected by the Cargo feature `wba_<major>_<minor>_<patch>`,
such as `wba_1_10_0`, where STM32WB's are `fw_*`.

## Workflow

```sh
# Validate the catalog and audit the annotations (no Cube clone needed).
cargo run -p stm32wb-catalog-extract -- audit

# Add a release: extract it with every existing one, then review the diff.
cargo run -p stm32wb-catalog-extract -- extract --add 1.25.0
git diff crates/stm32wb-catalog/catalog/stm32wb.toml

# Verify the checked-in catalog is what the tagged sources produce.
cargo run -p stm32wb-catalog-extract -- check

# List one feature set per distinct interface; CI tests stm32wb-hci with each.
cargo run -p stm32wb-catalog-extract -- targets

# The same commands maintain stm32wba.toml from ./STM32CubeWBA.
cargo run -p stm32wb-catalog-extract -- check --platform stm32wba
```

The extractor reads a local STM32CubeWB or STM32CubeWBA clone (default
`./STM32CubeWB` or `./STM32CubeWBA`) from Git objects only, so the clone's
worktree is never touched. It needs a loadable libclang (for example `libclang-dev` on Debian or Ubuntu).
