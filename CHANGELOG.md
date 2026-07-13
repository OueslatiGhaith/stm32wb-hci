## [Unreleased]

### 🚀 Features

- Support STM32CubeWB firmware 1.15.0, 1.16.0, 1.17.0, and 1.17.1 from one crate release through mutually exclusive firmware features.
- Add a feature-aware STM32CubeWB vendor-command/event compliance checker that discovers all
  declared firmware features, audits a checked-in exclusion policy, records resolved CubeWB tag
  commits, and drives CI without a hand-maintained firmware-version matrix.
- Extend compliance coverage to CubeWB standard HCI commands/events via the public `bt-hci`
  provider, and normalize vendor command requests, command returns, and event payloads into shared
  wire envelopes validated against the tagged C sources.
- Resolve generated CubeWB request-length formulas into bounded wire envelopes from their C
  parameter domains, branch-selected widths, and packed `sizeof` types.
- Resolve capacity-shaped CubeWB command returns into bounded wire envelopes from their packed C
  response structures.
- Let checked-in policy entries provide fixed or bounded payload evidence for transport-only
  events absent from CubeWB's generated catalog.
- Treat unavailable wire evidence as a compliance failure, locking in the zero-unavailable
  baseline for CI.
- Collapse unresolved catalog formulas and expressions into one fail-closed representation, and
  derive fixed request-type evidence from the shared packed-envelope parser.
- Keep CubeWB packed C type names private to the source adapter, parse their evidence once, and
  emit only resolved or explicitly unresolved catalog layouts; standard events remain
  inventory-only and carry no payload claim.
- Make event scope and payload evidence one schema-v7 enum, so vendor events require a payload and
  standard or LE Meta events cannot accidentally carry one.
- Make command identity scope-specific in the same catalog schema: vendor commands store an OCF,
  standard commands store an opcode, and standard OGF/OCF values are derived rather than repeated.
- Make command completion and return evidence one schema-v8 enum, normalize generated responses to
  command-owned return bytes at the source-adapter boundary, and make Command Complete without a
  return or Command Status with one unrepresentable in both catalog and Rust metadata.
- Refactor the host-only compliance tool around typed CLI, TOML, error, JSON, and C syntax-tree
  dependencies, replacing handwritten argument/manifest/JSON/C-structure parsing.

### 🐛 Bug Fixes

- Correct vendor opcode wiring for HAL link status and peripheral latency, and add missing HAL write-radio-register and GATT read-handle-value commands.
- Correct GAP Additional Beacon Set Data to encode its required one-byte data-length prefix.
- Cap GATT Read Handle Value responses at the 247 value bytes available in CubeWB's packed return
  structure.
- Correct the STM32WB command wire layouts for firmware build number, raw RSSI, GAP security
  level, GAP filter-accept-list configuration, and GATT Read Multiple Variable Characteristic
  Value; raw RSSI now preserves all three returned bytes and GAP security level exposes the
  firmware's mode/level pair. Correct L2CAP CoC Connect Confirm's request and completion
  response layout as well.

## [0.18.0] - 2026-05-24

### 🚀 Features

- rebase on top of bt-hci controller trait

### ⚙️ Miscellaneous Tasks

- Clippy
- Add embassy example
- Adjust example debug settings
- Add job for examples

## [0.17.4] - 2026-03-30

### ⚙️ Miscellaneous Tasks

- Bump version

## [0.17.3] - 2025-12-15

### 📚 Documentation

- Update CHANGELOG.md
- Update slugs

### ⚙️ Miscellaneous Tasks

- Bump version & update to rust 2024

## [0.17.2] - 2024-01-17

### 🐛 Bug Fixes

- Fixed syntax error in `crate::vendor::command::gatt::AccessPermission` using the `defmt` feature

## [0.17.1] - 2024-01-15

### 🚀 Features

- _(vendor event)_ Gap Pairing Complete event now returns a reason
- _(vendor GAP command)_ Add ADV Set Config command
- _(vendor GAP command)_ Add ADV Set Enable command
- _(vendor GAP command)_ Add ADV Set AAdvertising Data command
- _(vendor GAP command)_ Add ADV Set Scan Response Data command
- _(vendor GAP command)_ Add ADV Remove Set command
- _(vendor GAP command)_ Add ADV Clear Sets command
- _(vendor GAP command)_ Add ADV Set Random Address command
- _(vendor GATT command)_ Add Deny Read command
- _(vendor GATT command)_ Add Set Access Permission command
- _(vendor GATT command)_ Add Store Database command
- _(vendor GATT command)_ Add Send Multiple Notification command
- _(vendor GATT command)_ Add Read Multiple Variable Characteristic Value command
- _(vendor HAL command)_ Add Set Radio Activity Mask command
- _(vendor HAL command)_ Add Set Event Mask command
- _(vendor HAL command)_ Add Get PM Debug Info command
- _(vendor HAL command)_ Add Set Peripheral Latency command
- _(vendor HAL event)_ Add PM Debug Info event return parameters
- _(vendor HAL command)_ Add Read RSSI command
- _(vendor HAL command)_ Add Read Radio Register command
- _(vendor HAL command)_ Add Read Raw RSSI command
- _(vendor HAL command)_ Add RX Start command
- _(vendor HAL command)_ Add RX Stop command
- _(vendor HAL command)_ Add Stack Reset command
- _(LE event)_ Add LE Read Local P-256 Public Key Complete event
- _(LE event)_ Add LE Generated DH Key Complete event
- _(LE event)_ Add LE Enhanced Connection Complete event
- _(Vendor HAL Event)_ Add HAL End Of Radio Activity event
- _(Vendor HAL Event)_ Add HAL Scan Request Report event
- _(Vendor HAL Event)_ Add HAL Firmware Error event

### 📚 Documentation

- _(vendor GAP commad)_ Updated docs for GAP Clear Security command

## [0.17.0] - 2024-01-02

### 🚀 Features

- _(vendor event)_ Gatt EATT Bearer event
- _(vendor event)_ Add L2CAP COC Connect event
- _(vendor event)_ Add L2CAP COC Connect Cofirm event
- _(vendor event)_ Add L2CAP COC Reconfig event
- _(vendor event)_ Add L2CAP COC Reconfig Confirm event
- _(vendor event)_ Add L2CAP COC Disconnect event
- _(vendor event)_ Add L2CAP COC Flow Control event
- _(vendor event)_ Add L2CAP COC Rx Data event
- _(vendor L2CAP command)_ Added L2CAP COC Connect command
- _(vendor L2CAP command)_ Added L2CAP COC Connect Confirm command
- _(vendor L2CAP command)_ Added L2CAP COC Reconfig command
- _(vendor L2CAP command)_ Added L2CAP COC Reconfig Confirm command
- _(vendor L2CAP command)_ Added L2CAP COC Disconnect command
- _(vendor L2CAP command)_ Added L2CAP COC Flow Control command
- _(vendor L2CAP command)_ Added L2CAP COC Tx Data command
- _(vendor event)_ Add L2CAP COC Tx Pool Available event
- _(vendor event)_ Add GATT multi notification event
- _(vendor event)_ Add GATT Notification Complete event
- _(vendor event)_ Add GATT Read Ext event
- _(vendor event)_ Add GATT Indication Ext event
- _(vendor event)_ Add GATT Notification Ext event
- _(vendor GATT commands)_ Added Notify Notification Complete characterisitc event
- _(LE command)_ Add Set Controller To Host Flow Control command
- _(LE commad)_ Add Host Buffer Size commad
- _(LE command)_ Add Number of Completed Packets command

### 📚 Documentation

- Connection Handle range for Enhanced ATT bearer now ends at 0xEA3F
- HAL set config data parameters are updated

### ⚙️ Miscellaneous Tasks

- Update CI

## [0.16.0] - 2023-12-28

### 🚀 Features

- _(events)_ Change hardware error event parsing
- _(vendor event)_ Add values to Att Error types
- _(vendor gatt command)_ Update update_characteristic_value_ext

### 📚 Documentation

- _(vendor event)_ Update gap address not resolved event docs
- _(types)_ Update connection handle docs
- _(crate)_ Fix invalid references in docs
- _(vendor hal commands)_ Add docs to write_config_data
- _(vendor hal commands)_ Add docs to write_config_data

## [0.1.4] - 2023-12-27

### 🐛 Bug Fixes

- MAX_EVENT_LENGTH should be 256

## [ersion-0.1.3] - 2023-07-15

### 🚀 Features

- Make defmt optional

### 🐛 Bug Fixes

- Gap terminate general connection establishment procedure event
- Gatt discover char by type
- Gap pairing & numeric comparaison
- Gap set authentication requirements
- Gatt add char

### 💼 Other

- Extract uart module into its own file
- Update GAP commands to version 1.16.0
- Update commands to version 1.16.0
- Update vendor specific events to version 1.16.0
- Update vs events to version 1.16.0

### ⚙️ Miscellaneous Tasks

- Downgrade defmt
- Add defmt tests
- Remove version 4 tests
- Rename defmt test
- Remove defmt test
