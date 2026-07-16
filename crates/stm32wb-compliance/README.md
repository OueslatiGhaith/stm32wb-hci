# stm32wb-compliance

`stm32wb-compliance` is the host-side workspace tool that compares the declarative
`stm32wb-hci` API with generated STM32CubeWB sources. It is an internal
workspace tool; its catalog, report, and policy representations evolve together
with this repository and do not carry compatibility version fields.

## Requirements

The tool requires Git, a clang driver, and a loadable libclang installation (for
example, `libclang-dev` on Debian or Ubuntu). It materializes the BLE core tree
from the requested immutable CubeWB tag in a temporary directory, then uses
libclang to evaluate conditional branches with that tag's real include and
macro environment. The CubeWB checkout is never modified.

## Catalog and policy

Known payloads use `Evidence<WireLayout>`. A layout retains both its validated
length envelope and, when the generated source proves it, the ordered fixed and
variable wire segments. Variable segments preserve element width and
cardinality, so equal byte ranges no longer erase different field structures.
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
scope = "transport-event"
code = 0x9200
firmware = "*"
reason = "transport-only event | ordinary TOML strings may contain pipes"
payload = { minimum = 1, maximum = 1 }
```

Supported scopes are `command`, `event`, and `transport-event`.

- `firmware = "*"` selects every firmware declared by the crate; an exact
  version such as `"0.17.0"` selects one firmware.
- `command` and `event` exclusions must not contain `payload`.
- `transport-event` exclusions require a payload envelope measured after the
  two-byte vendor event code.
- Every reason must be non-empty.

The checker continues to reject duplicate or overlapping selectors, unknown
firmware versions, invalid event envelopes, and stale exclusions that do not
suppress a real difference. Use `--policy <path>` to test an alternate TOML
policy without changing the checked-in default.

## Commands

Options are owned by their subcommand and must appear after `check`, `diff`, or
`list-supported`. The former flags-before-subcommand compatibility is removed.

```sh
# Check one firmware and fail on differences.
cargo run -p stm32wb-compliance -- check --firmware 0.17.0 --deny

# Emit the machine-readable report.
cargo run -p stm32wb-compliance -- check --firmware 0.17.0 --json

# Compare two normalized catalogs and emit JSON.
cargo run -p stm32wb-compliance -- diff --from 0.16.0 --to 0.17.0 --json
```
