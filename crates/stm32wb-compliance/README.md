# stm32wb-compliance

`stm32wb-compliance` is the host-side workspace tool that compares the declarative
`stm32wb-hci` API with generated STM32CubeWB sources. It is not published as a
library, but its catalog JSON, report JSON, and checked-in exclusion policy are
machine-consumed compatibility contracts.

## Compatibility contracts

The tool has three independent version numbers. Consumers must check the version
for the artifact they are reading rather than treating the numbers as one global
tool version.

| Artifact                  | Version field       | Current version | Rust constant                    |
| ------------------------- | ------------------- | --------------- | -------------------------------- |
| Normalized CubeWB catalog | `schema_version`    | 9               | `CATALOG_SCHEMA_VERSION`         |
| Compliance check report   | `schema_version`    | 1               | `REPORT_SCHEMA_VERSION`          |
| Exclusion policy          | top-level `version` | 1               | internal `POLICY_FORMAT_VERSION` |

The `diff --json` envelope reports `catalog_schema_version`. A single
`check --json` report carries its report `schema_version` at the top level. For
`check --all-supported --json`, each successful `results[].report` carries its
own report `schema_version`.

Readers should reject unsupported versions. New optional data may be added
only when it preserves the documented contract; removing or renaming fields,
changing their meaning, or changing serialized enum shapes requires a version
increment.

## Migration: catalog schema 8 to 9

Schema 9 replaces the three parallel request, return, and event-payload layout
enums with one envelope representation:

```rust
enum ExtractedEnvelope {
    Known { minimum: u32, maximum: u32 },
    Unresolved(String),
}
```

Update catalog consumers using this mapping:

| Schema 8                        | Schema 9                           |
| ------------------------------- | ---------------------------------- |
| `RequestLayout::Empty`          | `Known { minimum: 0, maximum: 0 }` |
| `Fixed(n)`                      | `Known { minimum: n, maximum: n }` |
| `Variable { minimum, maximum }` | `Known { minimum, maximum }`       |
| `Unresolved(expression)`        | `Unresolved(expression)`           |

The serialized form therefore changes, for example, from:

```json
{ "kind": "fixed", "value": 3 }
```

to:

```json
{ "kind": "known", "value": { "minimum": 3, "maximum": 3 } }
```

Zero-length requests and returns now always serialize as a known `0..=0`
envelope; there is no separate `empty` state. The representation is used in
vendor command `request`, Command Complete `returns`, and vendor event
`payload` fields. Completion ownership is unchanged: only Command Complete
records contain a return envelope.

## Migration: compliance report schema 1

Compliance report JSON was previously unversioned. It now has a top-level
`"schema_version": 1` field. Consumers should require that field before reading
the rest of the report.

Schema 1 removes `catalog_counts.descriptor_command_ids`. Descriptor and active
command inventories were identical by construction, so keeping both could
represent an impossible disagreement. Use
`catalog_counts.active_command_ids` for the generated Rust command inventory;
there is no separate descriptor count.

No other report-count replacement is required. Vendor counts still describe
the CubeWB side, and active counts describe the selected Rust API.

## Migration: exclusion policy to TOML format 1

The old `exclusions.policy` pipe-delimited format version 2 has been removed.
The default policy is now [`exclusions.toml`](exclusions.toml), parsed as TOML
format version 1. Convert every old line into an `[[exclusions]]` table:

```toml
version = 1

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

```sh
# Check one firmware and fail on differences.
cargo run -p stm32wb-compliance -- check --firmware 0.17.0 --deny

# Emit the versioned report contract.
cargo run -p stm32wb-compliance -- check --firmware 0.17.0 --json

# Compare two normalized catalogs and emit catalog-schema-versioned JSON.
cargo run -p stm32wb-compliance -- diff --from 0.16.0 --to 0.17.0 --json
```
