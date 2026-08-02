# stm32wb-compliance

`stm32wb-compliance` is the host-side workspace tool that compares the declarative
`stm32wb-hci` API with one explicit STM32CubeWB wireless-binary target. It is an internal
workspace tool; its catalog, report, and policy representations evolve together
with this repository and do not carry compatibility version fields.

## Requirements

The tool requires Git, a clang driver, and a loadable libclang installation (for
example, `libclang-dev` on Debian or Ubuntu). It materializes the BLE core tree
from the requested immutable CubeWB tag in a temporary directory, then uses
libclang to evaluate conditional branches with that tag's real include and
macro environment. The CubeWB checkout is never modified.

## Catalog and policy

A compliance target is `{Cube release, MCU family, stack profile}`. The C adapter extracts a
complete release-level catalog and wire layouts from the shared generated wrappers. A separate
tagged-document adapter parses `STM32WB_BLE_Wireless_Interface.html` and filters that catalog by
its BF/PO/LO/LB/BO availability columns. The selected family's `Release_Notes.html` must map the
derived binary name to the requested profile, and the exact binary blob must exist at the tag.

The profile mapping is:

- `full-extended`: complete default interface;
- `full`: BF (Basic Features);
- `light`: PO (Peripheral Only);
- `hci-layer-extended`: LO (Link Layer Only);
- `hci-layer`: LB (Link Layer Only Basic);
- `hci-adv-scan`: BO (Beacon Only).

The availability join fails closed if a generated C entry has no matching documentation row, if
the documented name/code disagrees, if a table contains an unknown marker, or if the family
release notes do not establish the binary/profile mapping. System SHCI events remain sourced from
the tagged `shci.h` system interface rather than the BLE profile tables.

Known payloads use `Evidence<WireLayout>`. A layout retains both its validated
length envelope and, when the generated source proves it, the ordered fixed and
variable wire segments. Variable segments preserve element width and
cardinality, so equal byte ranges no longer erase different field structures.
Local `cmd!` standard-HCI extensions are parsed as well: their active `Params`
and `Return` types are compared with the matching generated opcode's request
and completion layouts, including packed local wire structs.
When a public API intentionally accepts fewer values than the generated buffer
can represent, its declarative field uses `storage_min_len` and/or
`storage_max_len` to record the complete transport range explicitly. Without
that opt-in, a narrowed variable field is a wire difference rather than a false
compliant result.
Unresolved source expressions remain `Evidence::Unresolved(String)`. Catalog
construction and deserialization also reject inconsistent layouts, duplicate
identities, out-of-range wire codes, and inconsistent names.

The checked-in [`exclusions.toml`](exclusions.toml) uses one `[[exclusions]]`
table for each policy entry:

```toml
[[exclusions]]
scope = "event"
code = 0x9203
firmware = "*"
reason = "OpenThread NVM notification is outside this Bluetooth HCI crate"
```

Supported scopes are `command` and `event`.

- `firmware = "*"` retains the policy's legacy field spelling and selects
  every Cube release declared by the crate; an exact version such as
  `"1.17.0"` selects one release.
- Optional `family` and `profile` selectors use the same values as the CLI and
  default to `"*"`; this keeps an exception scoped to the binary targets for
  which it is justified.
- Every reason must be non-empty.

The STM32WB adapter reads system-channel event IDs from the selected tag's
`SHCI_SUB_EVT_CODE_t` declaration in `shci.h`. Their payload evidence comes
from the same header, so exclusions select protocol scope only and never supply
otherwise invisible event metadata.

The checker continues to reject duplicate or overlapping selectors, unknown
Cube releases, and stale exclusions that do not suppress a real
difference. Use `--policy <path>` to test an alternate TOML policy without
changing the checked-in default.

## Commands

Options are owned by their subcommand and must appear after `check`, `diff`, or
`list-supported`. The former flags-before-subcommand compatibility is removed.

```sh
# Check one wireless binary and fail on differences.
cargo run -p stm32wb-compliance -- check --release 1.17.0 \
  --family wb5x --profile full-extended --deny

# Emit the machine-readable report.
cargo run -p stm32wb-compliance -- check --release 1.17.0 --json

# Compare two normalized catalogs and emit JSON.
cargo run -p stm32wb-compliance -- diff --from 1.16.0 --to 1.17.0 \
  --family wb5x --profile full-extended --json
```
