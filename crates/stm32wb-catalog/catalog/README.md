# STM32WB wireless-interface catalog

`stm32wb.toml` describes, for every supported STM32CubeWB release, what the
CPU2 wireless binaries accept and emit. It is the single source of protocol
facts for this repository. Both files are embedded in the `stm32wb-catalog`
crate, so its dependents are rebuilt whenever the extractor rewrites them.

## Layers

1. **Generated C.** Opcodes, completion kinds, and wire layouts are read from
   the tagged `ble_*_aci.c`, `ble_hci_le.c`, `ble_events.c`, `ble_types.h`,
   and `shci.h` with libclang. Relationships (which member counts a buffer,
   which member selects a union alternative) come from symbol references in
   the wrapper code, not from member names.
2. **ST documents.** Stack-profile availability comes from
   `STM32WB_BLE_Wireless_Interface.html`; which binaries exist for each MCU
   family comes from each family's `Release_Notes.html`.
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

Command returns include the leading status byte. Capacities are the element
capacities the generated C buffers declare.

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
```

The extractor reads a local STM32CubeWB clone (default `./STM32CubeWB`) from
Git objects only, so the clone's worktree is never touched. It needs a
loadable libclang (for example `libclang-dev` on Debian or Ubuntu).
