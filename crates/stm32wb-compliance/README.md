# stm32wb-compliance

`stm32wb-compliance` is the host-side workspace tool that compares the declarative
`stm32wb-hci` API with generated STM32CubeWB sources. It is an internal
workspace tool; its catalog, report, and policy representations evolve together
with this repository and do not carry compatibility version fields.

## Catalog and policy

Known wire lengths use `Evidence<Envelope>`. `Envelope` has private bounds and
can only be constructed when `minimum <= maximum`; unresolved source
expressions remain `Evidence::Unresolved(String)`. Catalog construction and
deserialization also reject duplicate identities, out-of-range wire codes, and
inconsistent names.

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
