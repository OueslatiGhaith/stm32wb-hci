## [0.17.3] - 2025-12-15

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
