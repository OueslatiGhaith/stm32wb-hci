//! GAP commands and types needed for those commands.

extern crate byteorder;

pub use crate::host::{AdvertisingFilterPolicy, AdvertisingType, OwnAddressType};
use crate::types::extended_advertisement::{
    AdvSet, AdvertisingEvent, AdvertisingOperation, AdvertisingPhy, ExtendedAdvertisingInterval,
};
pub use crate::types::{ConnectionInterval, ExpectedConnectionLength, ScanWindow};
use crate::vendor::command::BoundedItems;
#[cfg(after_fw_0_17_1)]
use crate::vendor::command::HciLengthError;
use crate::vendor::event::AttributeHandle;
use crate::vendor::event::command::{GapResolvePrivateAddress, GapSecurityLevel};
use crate::{AdvertisingHandle, BadStatusError, ConnectionHandle, Status};
pub use crate::{BdAddr, BdAddrType};
use crate::{
    host::{Channels, PeerAddrType, ScanFilterPolicy, ScanType},
    types::extended_advertisement::AdvertisingMode,
};
use bt_hci::cmd::{AsyncCmd, SyncCmd};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
#[cfg(after_fw_0_17_1)]
use byteorder::{ByteOrder, LittleEndian};
use core::time::Duration;

/// GAP-specific commands.
pub trait GapCommands {
    /// Set the device in non-discoverable mode. This command will disable the LL advertising and
    /// put the device in standby state.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetNonDiscoverable) event
    /// is generated.
    async fn gap_set_nondiscoverable(&self) -> Result<(), Error>;

    /// Set the device in limited discoverable mode.
    ///
    /// Limited discoverability is defined in in GAP specification volume 3, section 9.2.3. The
    /// device will be discoverable for maximum period of TGAP (lim_adv_timeout) = 180 seconds (from
    /// errata). The advertising can be disabled at any time by issuing a
    /// [`set_nondiscoverable`](GapCommands::gap_set_nondiscoverable) command.
    ///
    /// # Errors
    ///
    /// - [`BadAdvertisingType`](Error::BadAdvertisingType) if
    ///   [`advertising_type`](DiscoverableParameters::advertising_type) is one of the disallowed
    ///   types:
    ///   [ConnectableDirectedHighDutyCycle](crate::host::AdvertisingType::ConnectableDirectedHighDutyCycle)
    ///   or
    ///   [ConnectableDirectedLowDutyCycle](crate::host::AdvertisingType::ConnectableDirectedLowDutyCycle).
    /// - [`BadAdvertisingInterval`](Error::BadAdvertisingInterval) if
    ///   [`advertising_interval`](DiscoverableParameters::advertising_interval) is inverted.
    ///   That is, if the min is greater than the max.
    /// - [`BadConnectionInterval`](Error::BadConnectionInterval) if
    ///   [`conn_interval`](DiscoverableParameters::conn_interval) is inverted. That is, both the
    ///   min and max are provided, and the min is greater than the max.
    ///
    /// # Generated evenst
    ///
    /// When the controller receives the command, it will generate a [command status](crate::event::Event::CommandStatus)
    /// event. The controller starts the advertising after this and when advertising timeout happens
    /// (i.e. limited discovery period has elapsed), the controller generates an
    /// [GAP Limited Discoverable Complete](crate::vendor::event::VendorEvent::GapLimitedDiscoverableTimeout) event.
    async fn set_limited_discoverable(
        &self,
        params: &DiscoverableParameters<'_, '_>,
    ) -> Result<(), Error>;

    /// Set the device in discoverable mode.
    ///
    /// Limited discoverability is defined in in GAP specification volume 3, section 9.2.4. The
    /// device will be discoverable for maximum period of TGAP (lim_adv_timeout) = 180 seconds (from
    /// errata). The advertising can be disabled at any time by issuing a
    /// [`set_nondiscoverable`](GapCommands::gap_set_nondiscoverable) command.
    ///
    /// # Errors
    ///
    /// - [`BadAdvertisingType`](Error::BadAdvertisingType) if
    ///   [`advertising_type`](DiscoverableParameters::advertising_type) is one of the disallowed
    ///   types:
    ///   [ConnectableDirectedHighDutyCycle](crate::host::AdvertisingType::ConnectableDirectedHighDutyCycle)
    ///   or
    ///   [ConnectableDirectedLowDutyCycle](crate::host::AdvertisingType::ConnectableDirectedLowDutyCycle).
    /// - [`BadAdvertisingInterval`](Error::BadAdvertisingInterval) if
    ///   [`advertising_interval`](DiscoverableParameters::advertising_interval) is inverted.
    ///   That is, if the min is greater than the max.
    /// - [`BadConnectionInterval`](Error::BadConnectionInterval) if
    ///   [`conn_interval`](DiscoverableParameters::conn_interval) is inverted. That is, both the
    ///   min and max are provided, and the min is greater than the max.
    ///
    /// # Generated evenst
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetDiscoverable) event is
    /// generated.
    async fn set_discoverable(&self, params: &DiscoverableParameters<'_, '_>) -> Result<(), Error>;

    /// Set the device in direct connectable mode.
    ///
    /// Direct connectable mode is defined in GAP specification Volume 3,
    /// Section 9.3.3). Device uses direct connectable mode to advertise using either High Duty
    /// cycle advertisement events or Low Duty cycle advertisement events and the address as
    /// what is specified in the Own Address Type parameter. The Advertising Type parameter in
    /// the command specifies the type of the advertising used.
    ///
    /// When the `ms` feature is _not_ enabled, the device will be in directed connectable mode only
    /// for 1.28 seconds. If no connection is established within this duration, the device enters
    /// non discoverable mode and advertising will have to be again enabled explicitly.
    ///
    /// When the `ms` feature _is_ enabled, the advertising interval is explicitly provided in the
    /// [parameters][DirectConnectableParameters].
    ///
    /// # Errors
    ///
    /// - [`BadAdvertisingType`](Error::BadAdvertisingType) if
    ///   [`advertising_type`](DiscoverableParameters::advertising_type) is one of the disallowed
    ///   types:
    ///   [ConnectableUndirected](crate::host::AdvertisingType::ConnectableUndirected),
    ///   [ScannableUndirected](crate::host::AdvertisingType::ScannableUndirected), or
    ///   [NonConnectableUndirected](crate::host::AdvertisingType::NonConnectableUndirected),
    /// - (`ms` feature only) [`BadAdvertisingInterval`](Error::BadAdvertisingInterval) if
    ///   [`advertising_interval`](DiscoverableParameters::advertising_interval) is
    ///   out of range (20 ms to 10.24 s) or inverted (the min is greater than the max).
    ///
    /// # Generated evenst
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetDirectConnectable) event
    /// is generated.
    async fn set_direct_connectable(
        &self,
        params: &DirectConnectableParameters,
    ) -> Result<(), Error>;

    /// Set the IO capabilities of the device.
    ///
    /// This command has to be given only when the device is not in a connected state.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetIoCapability) event is
    /// generated.
    async fn set_io_capability(&self, capability: IoCapability) -> Result<(), Error>;

    /// Set the authentication requirements for the device.
    ///
    /// This command has to be given only when the device is not in a connected state.
    ///
    /// # Errors
    ///
    /// - [BadEncryptionKeySizeRange](Error::BadEncryptionKeySizeRange) if the
    ///   [`encryption_key_size_range`](AuthenticationRequirements::encryption_key_size_range) min
    ///   is greater than the max.
    /// - [BadFixedPin](Error::BadFixedPin) if the
    ///   [`fixed_pin`](AuthenticationRequirements::fixed_pin) is [Fixed](Pin::Fixed) with a value
    ///   greater than 999999.
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// - A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetAuthenticationRequirement) event
    ///   is generated.
    /// - If [`fixed_pin`](AuthenticationRequirements::fixed_pin) is [Request](Pin::Requested), then
    ///   a [GAP Pass Key](crate::vendor::event::VendorEvent::GapPassKeyRequest) event is generated.
    async fn set_authentication_requirement(
        &self,
        requirements: &AuthenticationRequirements,
    ) -> Result<(), Error>;

    /// Set the authorization requirements of the device.
    ///
    /// This command has to be given when connected to a device if authorization is required to
    /// access services which require authorization.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// - A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetAuthorizationRequirement) event
    ///   is generated.
    /// - If authorization is required, then a
    ///   [GAP Authorization Request](crate::vendor::event::VendorEvent::GapAuthorizationRequest)
    ///   event is generated.
    async fn set_authorization_requirement(
        &self,
        conn_handle: crate::ConnectionHandle,
        authorization_required: bool,
    ) -> Result<(), Error>;

    /// This command should be send by the host in response to the
    /// [GAP Pass Key Request](crate::vendor::event::VendorEvent::GapPassKeyRequest) event.
    ///
    /// `pin` contains the pass key which will be used during the pairing process.
    ///
    /// # Errors
    ///
    /// - [BadFixedPin](Error::BadFixedPin) if the pin is greater than 999999.
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// - A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapPassKeyResponse) event is
    ///   generated.
    /// - When the pairing process completes, it will generate a
    ///   [PairingComplete](crate::vendor::event::VendorEvent::GapPairingComplete) event.
    async fn pass_key_response(
        &self,
        conn_handle: crate::ConnectionHandle,
        pin: u32,
    ) -> Result<(), Error>;

    /// This command should be send by the host in response to the
    /// [GAP Authorization Request](crate::vendor::event::VendorEvent::GapAuthorizationRequest) event.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapAuthorizationResponse)
    /// event is generated.
    async fn authorization_response(
        &self,
        conn_handle: crate::ConnectionHandle,
        authorization: Authorization,
    ) -> Result<(), Error>;

    /// Register the GAP service with the GATT.
    ///
    /// The device name characteristic and appearance characteristic are added by default and the
    /// handles of these characteristics are returned in the
    /// [event data](crate::vendor::event::command::GapInit).
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapInit) event is generated.
    async fn init(
        &self,
        role: Role,
        privacy_enabled: bool,
        dev_name_characteristic_len: u8,
    ) -> Result<GapInit, Error>;

    /// Register the GAP service with the GATT.
    ///
    /// This function exists to prevent name conflicts with other Commands traits' init methods.
    async fn init_gap(
        &self,
        role: Role,
        privacy_enabled: bool,
        dev_name_characteristic_len: u8,
    ) -> Result<GapInit, Error> {
        self.init(role, privacy_enabled, dev_name_characteristic_len)
            .await
    }

    /// Put the device into non-connectable mode.
    ///
    /// This mode does not support connection. The privacy setting done in the
    /// [`init`](GapCommands::init) command plays a role in deciding the valid
    /// parameters for this command. If privacy was not enabled, `address_type` may be
    /// [Public](AddressType::Public) or [Random](AddressType::Random).  If privacy was
    /// enabled, `address_type` may be [ResolvablePrivate](AddressType::ResolvablePrivate) or
    /// [NonResolvablePrivate](AddressType::NonResolvablePrivate).
    ///
    /// # Errors
    ///
    /// - [BadAdvertisingType](Error::BadAdvertisingType) if the advertising type is not one
    ///   of the supported modes. It must be
    ///   [ScannableUndirected](AdvertisingType::ScannableUndirected) or
    ///   [NonConnectableUndirected](AdvertisingType::NonConnectableUndirected).
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::Status) event is generated.
    async fn set_nonconnectable(
        &self,
        advertising_type: AdvertisingType,
        address_type: AddressType,
    ) -> Result<(), Error>;

    /// Put the device into undirected connectable mode.
    ///
    /// The privacy setting done in the [`init`](GapCommands::init) command plays a role
    /// in deciding the valid parameters for this command.
    ///
    /// # Errors
    ///
    /// - [BadAdvertisingFilterPolicy](Error::BadAdvertisingFilterPolicy) if the filter is
    ///   not one of the supported modes. It must be
    ///   [AllowConnectionAndScan](AdvertisingFilterPolicy::AllowConnectionAndScan) or
    ///   [WhiteListConnectionAllowScan](AdvertisingFilterPolicy::WhiteListConnectionAllowScan).
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetUndirectedConnectable)
    /// event is generated.
    async fn set_undirected_connectable(
        &self,
        params: &UndirectedConnectableParameters,
    ) -> Result<(), Error>;

    /// This command has to be issued to notify the central device of the security requirements of
    /// the peripheral.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event will be generated when a valid
    /// command is received. On completion of the command, i.e. when the security request is
    /// successfully transmitted to the master, a
    /// [GAP Peripheral Security Initiated](crate::vendor::event::VendorEvent::GapPeripheralSecurityInitiated)
    /// vendor-specific event will be generated.
    async fn peripheral_security_request(
        &self,
        conn_handle: &ConnectionHandle,
    ) -> Result<(), Error>;

    /// This command can be used to update the advertising data for a particular AD type. If the AD
    /// type specified does not exist, then it is added to the advertising data. If the overall
    /// advertising data length is more than 31 octets after the update, then the command is
    /// rejected and the old data is retained.
    ///
    /// # Errors
    ///
    /// - [BadAdvertisingDataLength](Error::BadAdvertisingDataLength) if the provided data is longer
    ///   than 31 bytes.
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapUpdateAdvertisingData)
    /// event is generated.
    async fn update_advertising_data(&self, data: &[u8]) -> Result<(), Error>;

    /// This command can be used to delete the specified AD type from the advertisement data if
    /// present.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapDeleteAdType) event is
    /// generated.
    async fn delete_ad_type(&self, ad_type: AdvertisingDataType) -> Result<(), Error>;

    /// This command can be used to get the current security settings of the device.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapGetSecurityLevel) event is
    /// generated.
    async fn get_security_level(
        &self,
        conn_handle: &ConnectionHandle,
    ) -> Result<GapSecurityLevel, Error>;

    /// Allows masking events from the GAP.
    ///
    /// The default configuration is all the events masked.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapSetEventMask) event is
    /// generated.
    async fn set_event_mask(&self, flags: EventFlags) -> Result<(), Error>;

    /// Allows masking events from the GAP.
    ///
    /// This function exists to prevent name conflicts with other Commands traits' set_event_mask
    /// methods.
    async fn set_gap_event_mask(&self, flags: EventFlags) -> Result<(), Error> {
        self.set_event_mask(flags).await
    }

    /// Configure the controller's white list with devices that are present in the security
    /// database.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapConfigureWhiteList) event
    /// is generated.
    async fn configure_white_list(&self) -> Result<(), Error>;

    /// Command the controller to terminate the connection.
    ///
    /// # Errors
    ///
    /// - [BadTerminationReason](Error::BadTerminationReason) if provided termination reason is
    ///   invalid. Valid reasons are the same as HCI [disconnect](crate::host::HostHci::disconnect):
    ///   [`AuthFailure`](crate::Status::AuthFailure),
    ///   [`RemoteTerminationByUser`](crate::Status::RemoteTerminationByUser),
    ///   [`RemoteTerminationLowResources`](crate::Status::RemoteTerminationLowResources),
    ///   [`RemoteTerminationPowerOff`](crate::Status::RemoteTerminationPowerOff),
    ///   [`UnsupportedRemoteFeature`](crate::Status::UnsupportedRemoteFeature),
    ///   [`PairingWithUnitKeyNotSupported`](crate::Status::PairingWithUnitKeyNotSupported), or
    ///   [`UnacceptableConnectionParameters`](crate::Status::UnacceptableConnectionParameters).
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// The controller will generate a [command status](crate::event::Event::CommandStatus) event when
    /// the command is received and a [Disconnection Complete](crate::event::Event::DisconnectionComplete)
    /// event will be generated when the link is
    /// disconnected.
    async fn terminate(
        &self,
        conn_handle: crate::ConnectionHandle,
        reason: crate::Status,
    ) -> Result<(), Error>;

    /// Clear the bonding table. All the devices in the bonding table are removed.
    ///
    /// See also [remove_bonded_device](GapCommands::remove_bonded_device) to remove only one device.
    ///
    /// # Note
    /// As a fallback mode, in case the bonding table is full, the BLE stack automatically clears the bonding
    /// table just before putting into it information about a new bonded device.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapClearSecurityDatabase)
    /// event is generated.
    async fn clear_security_database(&self) -> Result<(), Error>;

    /// This command should be given by the application when it receives the
    /// [GAP Bond Lost](crate::vendor::event::VendorEvent::GapBondLost) event if it wants the re-bonding to happen
    /// successfully. If this command is not given on receiving the event, the bonding procedure
    /// will timeout.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [Command Complete](crate::vendor::event::command::VendorReturnParameters::GapAllowRebond) event is
    /// generated. Even if the command is given when it is not valid, success will be returned but
    /// internally it will have no effect.
    async fn allow_rebond(&self, conn_handle: crate::ConnectionHandle) -> Result<(), Error>;

    /// Start the limited discovery procedure.
    ///
    /// The controller is commanded to start active scanning.  When this procedure is started, only
    /// the devices in limited discoverable mode are returned to the upper layers.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event is generated as soon as the
    /// command is given.
    ///
    /// If [Success](crate::Status::Success) is returned in the command status, the procedure is
    /// terminated when either the upper layers issue a command to terminate the procedure by
    /// issuing the command [`terminate_procedure`](GapCommands::terminate_gap_procedure) with the
    /// procedure code set to [LimitedDiscovery](crate::vendor::event::GapProcedure::LimitedDiscovery) or a
    /// [timeout](crate::vendor::event::VendorEvent::GapLimitedDiscoverableTimeout) happens. When the
    /// procedure is terminated due to any of the above reasons, a
    /// [ProcedureComplete](crate::vendor::event::VendorEvent::GapProcedureComplete) event is returned with
    /// the procedure code set to [LimitedDiscovery](crate::vendor::event::GapProcedure::LimitedDiscovery).
    ///
    /// The device found when the procedure is ongoing is returned to the upper layers through the
    /// [LeAdvertisingReport](crate::event::Event::LeAdvertisingReport) event.
    async fn start_limited_discovery_procedure(
        &self,
        params: &DiscoveryProcedureParameters,
    ) -> Result<(), Error>;

    /// Start the general discovery procedure. The controller is commanded to start active scanning.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event is generated as soon as the
    /// command is given.
    ///
    /// If [Success](crate::Status::Success) is returned in the command status, the procedure is
    /// terminated when either the upper layers issue a command to terminate the procedure by
    /// issuing the command [`terminate_procedure`](GapCommands::terminate_gap_procedure) with the
    /// procedure code set to [GeneralDiscovery](crate::vendor::event::GapProcedure::GeneralDiscovery) or a
    /// timeout happens. When the procedure is terminated due to any of the above reasons, a
    /// [ProcedureComplete](crate::vendor::event::VendorEvent::GapProcedureComplete) event is returned with
    /// the procedure code set to [GeneralDiscovery](crate::vendor::event::GapProcedure::GeneralDiscovery).
    ///
    /// The device found when the procedure is ongoing is returned to the upper layers through the
    /// [LeAdvertisingReport](crate::event::Event::LeAdvertisingReport) event.
    async fn start_general_discovery_procedure(
        &self,
        params: &DiscoveryProcedureParameters,
    ) -> Result<(), Error>;

    /// Start the auto connection establishment procedure.
    ///
    /// The devices specified are added to the white list of the controller and a
    /// [`le_create_connection`](crate::host::HostHci::le_create_connection) call will be made to the
    /// controller by GAP with the [initiator filter policy](crate::host::ConnectionParameters::initiator_filter_policy) set to
    /// [WhiteList](crate::host::ConnectionFilterPolicy::WhiteList), to "use whitelist to determine
    /// which advertiser to connect to". When a command is issued to terminate the procedure by
    /// upper layer, a [`le_create_connection_cancel`](crate::host::HostHci::le_create_connection_cancel)
    /// call will be made to the controller by GAP.
    ///
    /// # Errors
    ///
    /// - If the [`white_list`](AutoConnectionEstablishmentParameters::white_list) is too long
    ///   (such that the serialized command would not fit in 255 bytes), a
    ///   [WhiteListTooLong](Error::WhiteListTooLong) is returned. The list cannot have more than 33
    ///   elements.
    async fn start_auto_connection_establishment_procedure(
        &self,
        params: &AutoConnectionEstablishmentParameters<'_>,
    ) -> Result<(), Error>;

    /// Start a general connection establishment procedure.
    ///
    /// The host [enables scanning](crate::host::HostHci::le_set_scan_enable) in the controller with the
    /// scanner [filter policy](crate::host::ScanParameters::filter_policy) set to
    /// [AcceptAll](crate::host::ScanFilterPolicy::AcceptAll), to "accept all advertising packets" and
    /// from the scanning results, all the devices are sent to the upper layer using the event
    /// [LE Advertising Report](crate::event::Event::LeAdvertisingReport). The upper layer then has to
    /// select one of the devices to which it wants to connect by issuing the command
    /// [`create_connection`](GapCommands::create_connection). If privacy is enabled,
    /// then either a private resolvable address or a non-resolvable address, based on the address
    /// type specified in the command is set as the scanner address but the GAP create connection
    /// always uses a private resolvable address if the general connection establishment procedure
    /// is active.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    async fn start_general_connection_establishment_procedure(
        &self,
        params: &GeneralConnectionEstablishmentParameters,
    ) -> Result<(), Error>;

    /// Start a selective connection establishment procedure.
    ///
    /// The GAP adds the specified device addresses into white list and
    /// [enables scanning](crate::host::HostHci::le_set_scan_enable) in the controller with the scanner
    /// [filter policy](crate::host::ScanParameters::filter_policy) set to
    /// [WhiteList](crate::host::ScanFilterPolicy::WhiteList), to "accept packets only from devices in
    /// whitelist". All the devices found are sent to the upper layer by the event
    /// [LE Advertising Report](crate::event::Event::LeAdvertisingReport). The upper layer then has to select one of
    /// the devices to which it wants to connect by issuing the command
    /// [`create_connection`](GapCommands::create_connection).
    ///
    /// # Errors
    ///
    /// - If the [`white_list`](SelectiveConnectionEstablishmentParameters::white_list) is too
    ///   long (such that the serialized command would not fit in 255 bytes), a
    ///   [WhiteListTooLong](Error::WhiteListTooLong) is returned. The list cannot have more than 35
    ///   elements.
    async fn start_selective_connection_establishment_procedure(
        &self,
        params: &SelectiveConnectionEstablishmentParameters<'_>,
    ) -> Result<(), Error>;

    /// Start the direct connection establishment procedure.
    ///
    /// A [LE Create Connection](crate::host::HostHci::le_create_connection) call will be made to the
    /// controller by GAP with the initiator [filter policy](crate::host::ConnectionParameters::initiator_filter_policy) set to
    /// [UseAddress](crate::host::ConnectionFilterPolicy::UseAddress) to "ignore whitelist and process
    /// connectable advertising packets only for the specified device". The procedure can be
    /// terminated explicitly by the upper layer by issuing the command
    /// [`terminate_procedure`](GapCommands::terminate_gap_procedure). When a command is
    /// issued to terminate the procedure by upper layer, a
    /// [`le_create_connection_cancel`](crate::host::HostHci::le_create_connection_cancel) call will be
    /// made to the controller by GAP.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event is generated as soon as the
    /// command is given. If [Success](crate::Status::Success) is returned, on termination of the
    /// procedure, a [LE Connection Complete](crate::event::LeConnectionComplete) event is
    /// returned. The procedure can be explicitly terminated by the upper layer by issuing the
    /// command [`terminate_procedure`](GapCommands::terminate_gap_procedure) with the procedure_code set
    /// to
    /// [DirectConnectionEstablishment](crate::vendor::event::GapProcedure::DirectConnectionEstablishment).
    async fn create_connection(&self, params: &ConnectionParameters) -> Result<(), Error>;

    /// The GAP procedure(s) specified is terminated.
    ///
    /// # Errors
    ///
    /// - [NoProcedure](Error::NoProcedure) if the bitfield is empty.
    /// - Underlying communication errors
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapTerminateProcedure) event
    /// is generated for this command. If the command was successfully processed, the status field
    /// will be [Success](crate::Status::Success) and a
    /// [ProcedureCompleted](crate::vendor::event::VendorEvent::GapProcedureComplete) event is returned
    /// with the procedure code set to the corresponding procedure.
    async fn terminate_gap_procedure(&self, procedure: Procedure) -> Result<(), Error>;

    /// Start the connection update procedure.
    ///
    /// A [`le_connection_update`](crate::host::HostHci::le_connection_update) call is be made to the
    /// controller by GAP.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event is generated as soon as the
    /// command is given. If [Success](crate::Status::Success) is returned, on completion of
    /// connection update, a
    /// [LeConnectionUpdateComplete](crate::event::Event::LeConnectionUpdateComplete) event is
    /// returned to the upper layer.
    async fn start_connection_update(
        &self,
        params: &ConnectionUpdateParameters,
    ) -> Result<(), Error>;

    /// Send the SM pairing request to start a pairing process. The authentication requirements and
    /// I/O capabilities should be set before issuing this command using the
    /// [`set_io_capability`](GapCommands::set_io_capability) and
    /// [`set_authentication_requirement`](GapCommands::set_authentication_requirement)
    /// commands.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command status](crate::event::Event::CommandStatus) event is generated when the command is
    /// received. If [Success](crate::Status::Success) is returned in the command status event, a
    /// [Pairing Complete](crate::vendor::event::VendorEvent::GapPairingComplete) event is returned after
    /// the pairing process is completed.
    async fn send_pairing_request(&self, params: &PairingRequest) -> Result<(), Error>;

    /// This command tries to resolve the address provided with the IRKs present in its database.
    ///
    /// If the address is resolved successfully with any one of the IRKs present in the database, it
    /// returns success and also the corresponding public/static random address stored with the IRK
    /// in the database.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapResolvePrivateAddress)
    /// event is generated. If [Success](crate::Status::Success) is returned as the status, then the
    /// address is also returned in the event.
    async fn resolve_private_address(
        &self,
        addr: crate::BdAddr,
    ) -> Result<GapResolvePrivateAddress, Error>;

    /// This command puts the device into broadcast mode.
    ///
    /// # Errors
    ///
    /// - [BadAdvertisingType](Error::BadAdvertisingType) if the advertising type is not
    ///   [ScannableUndirected](crate::types::AdvertisingType::ScannableUndirected) or
    ///   [NonConnectableUndirected](crate::types::AdvertisingType::NonConnectableUndirected).
    /// - [BadAdvertisingDataLength](Error::BadAdvertisingDataLength) if the advertising data is
    ///   longer than 31 bytes.
    /// - [WhiteListTooLong](Error::WhiteListTooLong) if the length of the white list would put the
    ///   packet length over 255 bytes. The exact number of addresses that can be in the white list
    ///   can range from 35 to 31, depending on the length of the advertising data.
    /// - Underlying communication errors.
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapSetBroadcastMode) event is
    /// returned where the status indicates whether the command was successful.
    async fn set_broadcast_mode(&self, params: &BroadcastModeParameters) -> Result<(), Error>;

    /// Starts an Observation procedure, when the device is in Observer Role.
    ///
    /// The host enables scanning in the controller. The advertising reports are sent to the upper
    /// layer using standard LE Advertising Report Event. See Bluetooth Core v4.1, Vol. 2, part E,
    /// Ch. 7.7.65.2, LE Advertising Report Event.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapStartObservationProcedure)
    /// event is generated.
    async fn start_observation_procedure(
        &self,
        params: &ObservationProcedureParameters,
    ) -> Result<(), Error>;

    /// This command gets the list of the devices which are bonded. It returns the number of
    /// addresses and the corresponding address types and values.
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapGetBondedDevices) event is
    /// generated.
    async fn get_bonded_devices(&self) -> Result<GapBondedDevices, Error>;

    /// The command finds whether the device, whose address is specified in the command, is
    /// bonded. If the device is using a resolvable private address and it has been bonded, then the
    /// command will return [Success](crate::Status::Success).
    ///
    /// # Errors
    ///
    /// Only underlying communication errors are reported.
    ///
    /// # Generated events
    ///
    /// A [command complete](crate::vendor::event::command::VendorReturnParameters::GapIsDeviceBonded) event is
    /// generated.
    async fn is_device_bonded(&self, addr: crate::host::PeerAddrType) -> Result<(), Error>;

    /// This command allows the user to validate/confirm or not the numeric comparison value showed through
    /// the [`GapNumericComparisonValue`](crate::vendor::event::GapNumericComparisonValue) event.
    async fn numeric_comparison_value_confirm_yes_no(
        &self,
        params: &NumericComparisonValueConfirmYesNoParameters,
    ) -> Result<(), Error>;

    /// This command permits to signal to the Stack the input type detected during Passkey input.
    async fn passkey_input(
        &self,
        conn_handle: ConnectionHandle,
        input_type: InputType,
    ) -> Result<(), Error>;

    /// This command is sent by the user to get (i.e. to extract from the Stack) the OOB
    /// data generated by the Stack itself.
    async fn get_oob_data(&self, oob_data_type: OobDataType) -> Result<[u8; 26], Error>;

    /// This command is sent (by the User) to input the OOB data arrived via OOB
    /// communication.
    async fn set_oob_data(&self, params: &SetOobDataParameters) -> Result<(), Error>;

    /// This  command is used to add devices to the list of address translations
    /// used to resolve Resolvable Private Addresses in the Controller.
    async fn add_devices_to_resolving_list(
        &self,
        whitelist_identities: &[PeerAddrType],
        clear_resolving_list: bool,
    ) -> Result<(), Error>;

    /// This command is used to remove a specified device from bonding table
    async fn remove_bonded_device(&self, address: BdAddrType) -> Result<(), Error>;

    /// This  command is used to add specific device addresses to the white and/or resolving list.
    async fn add_devices_to_list(
        &self,
        list_entries: &[BdAddrType],
        mode: AddDeviceToListMode,
    ) -> Result<(), Error>;

    /// This command starts an advertising beacon. It allows additional advertising
    /// packets to be transmitted independently of the packets transmitted with GAP
    /// advertising commands such as ACI_GAP_SET_DISCOVERABLE or
    /// ACI_GAP_SET_LIMITED_DISCOVERABLE.
    async fn additional_beacon_start(
        &self,
        params: &AdditonalBeaconStartParameters,
    ) -> Result<(), Error>;

    /// This command stops the advertising beacon started with
    /// ACI_GAP_ADDITIONAL_BEACON_START.
    async fn additional_beacon_stop(&self) -> Result<(), Error>;

    /// This command sets the data transmitted by the advertising beacon started
    /// with ACI_GAP_ADDITIONAL_BEACON_START. If the advertising beacon is already
    /// started, the new data is used in subsequent beacon advertising events.
    async fn additonal_beacon_set_data(&self, advertising_data: &[u8]) -> Result<(), Error>;

    /// This command is used to set the extended advertising configuration for one
    /// advertising set.
    ///
    /// This command, in association with
    /// [adv_set_scan_response_data](GapCommands::adv_set_scan_response_data),
    /// [adv_set_advertising_data](GapCommands::adv_set_advertising_data) and
    /// [adv_set_enable](GapCommands::adv_set_enable), enables to start extended
    /// advertising.
    ///
    /// These commands must be used in replacement of
    /// [set_discoverable](GapCommands::set_discoverable),
    /// [set_limited_discoverable](GapCommands::set_limited_discoverable),
    /// [set_direct_connectable](GapCommands::set_direct_connectable),
    /// [set_nonconnectable](GapCommands::set_nonconnectable),
    /// [set_undirected_connectable](GapCommands::set_undirected_connectable) and
    /// [set_broadcast_mode](GapCommands::set_broadcast_mode) that only support
    /// legacy advertising.
    async fn adv_set_config(&self, params: &AdvSetConfig) -> Result<(), Error>;

    /// This command is used to request the Controller to enable or disbale one
    /// or more extended advertising sets.
    async fn adv_set_enable<'a>(&self, params: &AdvSetEnable<'a>) -> Result<(), Error>;

    /// This command is used to set the data used in extended advertising PDUs
    /// that have a data field
    async fn adv_set_advertising_data(&self, params: &AdvSetAdvertisingData) -> Result<(), Error>;

    /// This command is used to provide scan response data used during extended
    /// advertising
    async fn adv_set_scan_response_data(&self, params: &AdvSetAdvertisingData)
    -> Result<(), Error>;

    /// This command is used to remove an advertising set from the Controller.
    async fn adv_remove_set(&self, handle: AdvertisingHandle) -> Result<(), Error>;

    /// This command is used to remove all exisiting advertising sets from
    /// the Controller.
    async fn adv_clear_sets(&self) -> Result<(), Error>;

    /// This command is used to set the random device address of an advertising
    /// set configured to use specific random address.
    async fn adv_set_random_address(
        &self,
        handle: AdvertisingHandle,
        addr: BdAddr,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Reply to ACI_GAP_PAIRING_REQUEST_EVENT to accept or reject pairing.
    async fn pairing_request_reply(
        &self,
        conn_handle: crate::ConnectionHandle,
        accept: bool,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Set parameters for periodic advertising.
    async fn adv_set_periodic_parameters(
        &self,
        params: &AdvSetPeriodicParameters,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Set data for periodic advertising PDUs.
    async fn adv_set_periodic_data<'a>(&self, params: &AdvSetPeriodicData<'a>)
    -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Enable or disable periodic advertising.
    async fn adv_set_periodic_enable(
        &self,
        enable: u8,
        handle: AdvertisingHandle,
    ) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Set extended advertising configuration (V2 with 4-byte intervals and PHY options).
    async fn adv_set_configuration_v2(&self, params: &AdvSetConfigV2) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Start extended scan procedure.
    async fn ext_start_scan(&self, params: &ExtStartScanParams) -> Result<(), Error>;

    #[cfg(after_fw_0_17_1)]
    /// Create connection using extended advertising.
    async fn ext_create_connection(&self, params: &ExtCreateConnectionParams) -> Result<(), Error>;
}

vendor_cmd! {
    GapSetNonDiscoverable(GAP_SET_NONDISCOVERABLE) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetLimitedDiscoverable(GAP_SET_LIMITED_DISCOVERABLE) {
        Params<'a> = {
            advertising_type: u8 => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSetDiscoverable(GAP_SET_DISCOVERABLE) {
        Params<'a> = {
            advertising_type: u8 => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetDirectConnectable(GAP_SET_DIRECT_CONNECTABLE) {
        Params = {
            own_address_type: u8 => 1,
            advertising_type: u8 => 1,
            initiator_address: BdAddrType => 7,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetIoCapability(GAP_SET_IO_CAPABILITY) {
        Params = {
            io_capability: IoCapability => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetAuthenticationRequirement(GAP_SET_AUTHENTICATION_REQUIREMENT) {
        Params = {
            bonding_required: bool => 1,
            mitm_protection_required: bool => 1,
            secure_connection_support: u8 => 1,
            keypress_notification_support: bool => 1,
            encryption_key_size_min: u8 => 1,
            encryption_key_size_max: u8 => 1,
            pass_key_required: bool => 1,
            fixed_pin: u32 => 4,
            identity_address_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetAuthorizationRequirement(GAP_SET_AUTHORIZATION_REQUIREMENT) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            authorization_required: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPassKeyResponse(GAP_PASS_KEY_RESPONSE) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            pin: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAuthorizationResponse(GAP_AUTHORIZATION_RESPONSE) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            authorization: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

// TODO: verify these return parameters

vendor_cmd! {
    CmdGapInit(GAP_INIT) {
        Params = {
            role: Role => 1,
            privacy_enabled: bool => 1,
            dev_name_characteristic_len: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GapInit {
            service_handle: AttributeHandle => 2,
            dev_name_handle: AttributeHandle => 2,
            appearance_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GapSetNonConnectable(GAP_SET_NONCONNECTABLE) {
        Params = {
            advertising_type: u8 => 1,
            address_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetUnidirectedConnectable(GAP_SET_UNDIRECTED_CONNECTABLE) {
        Params = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPeripheralSecurityRequest(GAP_PERIPHERAL_SECURITY_REQUEST) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapUpdateAdvertisingData(GAP_UPDATE_ADVERTISING_DATA) {
        Params<'a> = {
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapDeleteAdType(GAP_DELETE_AD_TYPE) {
        Params = {
            ad_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapGetSecurityLevel(GAP_GET_SECURITY_LEVEL) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandComplete;
        Return = GapSecurityLevelReturn {
            security_mode: u8 => 1,
            security_level: u8 => 1,
        };
    }
}

vendor_cmd! {
    GapSetEventMask(GAP_SET_EVENT_MASK) {
        Params = {
            flags: u16 => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapConfigureWhitelist(GAP_CONFIGURE_WHITE_LIST) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapTerminate(GAP_TERMINATE) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            reason: u8 => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapClearSecurityDatabase(GAP_CLEAR_SECURITY_DATABASE) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAllowRebond(GAP_ALLOW_REBOND) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartLimitedDiscoveryProcedure(GAP_START_LIMITED_DISCOVERY_PROCEDURE) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralDiscoveryProcedure(GAP_START_GENERAL_DISCOVERY_PROCEDURE) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartAutoConnectionEstablishmentProcedure(GAP_START_AUTO_CONNECTION_ESTABLISHMENT) {
        Params<'a> = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 33,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralConnectionEstablishmentProcedure(GAP_START_GENERAL_CONNECTION_ESTABLISHMENT) {
        Params = {
            scan_type: u8 => 1,
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            filter_policy: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartSelectiveConnectionEstablishmentProcedure(GAP_START_SELECTIVE_CONNECTION_ESTABLISHMENT) {
        Params<'a> = {
            scan_type: u8 => 1,
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
            filter_duplicates: bool => 1,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 35,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapCreateConnection(GAP_CREATE_CONNECTION) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            peer_address: PeerAddrType => 7,
            own_address_type: u8 => 1,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapTerminateProcedure(GAP_TERMINATE_PROCEDURE) {
        Params = {
            procedure: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartConnectionUpdate(GAP_START_CONNECTION_UPDATE) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSendPairingRequest(GAP_SEND_PAIRING_REQUEST) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            force_rebond: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    CmdGapResolvePrivateAddress(GAP_RESOLVE_PRIVATE_ADDRESS) {
        Params = {
            address: BdAddr => 6,
        };
        Completion = CommandComplete;
        Return = GapResolvedPrivateAddress {
            address: BdAddr => 6,
        };
    }
}

vendor_cmd! {
    GapSetBroadcastMode(GAP_SET_BROADCAST_MODE) {
        Params<'a> = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            advertising_type: u8 => 1,
            own_address_type: u8 => 1,
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 35,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartObservationProcedure(GAP_START_OBSERVATION_PROCEDURE) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            scan_type: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
            filter_policy: u8 => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapGetBondedDevices(GAP_GET_BONDED_DEVICES) {
        Params = ();
        Completion = CommandComplete;
        Return = GapBondedDevices {
            addresses: BoundedItems<BdAddrType, 35> => {
                kind: counted_items,
                count: u8 => 1,
                item: BdAddrType => 7,
                max_items: 35,
            },
        };
    }
}

impl GapBondedDevices {
    pub(crate) const MAX_ADDRESSES: usize = 35;

    /// Addresses reported by the controller.
    pub fn bonded_addresses(&self) -> &[BdAddrType] {
        self.addresses.as_slice()
    }
}

vendor_cmd! {
    GapIsDeviceBonded(GAP_IS_DEVICE_BONDED) {
        Params = {
            address: PeerAddrType => 7,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapConfirmNumericComparisonValue(GAP_NUMERIC_COMPARISON_VALUE_YES_NO) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            confirm_yes_no: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPasskeyInput(GAP_PASSKEY_INPUT) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            input_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}
vendor_cmd! {
    GapGetOobData(GAP_GET_OOB_DATA) {
        Params = {
            oob_data_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GapOobData {
            address_type: u8 => 1,
            address: BdAddr => 6,
            oob_data_type: u8 => 1,
            oob_data_len: u8 => 1,
            oob_data: [u8; 16] => 16,
        };
    }
}

vendor_cmd! {
    GapSetOobData(GAP_SET_OOB_DATA) {
        Params = {
            device_type: u8 => 1,
            address: BdAddrType => 7,
            oob_data_type: u8 => 1,
            oob_data_len: u8 => 1,
            oob_data: [u8; 16] => 16,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAddDevicesToResolvingList(GAP_ADD_DEVICES_TO_RESOLVING_LIST) {
        Params<'a> = {
            whitelist_identities: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 36,
            },
            clear_resolving_list: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapRemoveBondedDevice(GAP_REMOVE_BONDED_DEVICE) {
        Params = {
            address: BdAddrType => 7,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAddDevicesToList(GAP_ADD_DEVICES_TO_LIST) {
        Params<'a> = {
            list_entries: &'a [BdAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: BdAddrType => 7,
                max_items: 36,
            },
            mode: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconStart(GAP_ADDITIONAL_BEACON_START) {
        Params = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            advertising_channel_map: u8 => 1,
            own_address_type: BdAddrType => 7,
            pa_level: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconStop(GAP_ADDITIONAL_BEACON_STOP) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconSetData(GAP_ADDITIONAL_BEACON_SET_DATA) {
        Params<'a> = {
            advertising_data: &'a [u8] => {
                kind: trailing_bytes,
                min_len: 0,
                max_len: 255,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetConfig(GAP_ADV_SET_CONFIGURATION) {
        Params<'a> = {
            adv_mode: u8 => 1,
            adv_handle: AdvertisingHandle => 1,
            adv_event_properties: u16 => 2,
            adv_interval: &'a ExtendedAdvertisingInterval => 8,
            primary_adv_channel_map: u8 => 1,
            own_addr_type: u8 => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: u8 => 1,
            adv_tx_power: u8 => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: u8 => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetEnable(GAP_ADV_SET_ENABLE) {
        Params<'a> = {
            enable: bool => 1,
            adv_set: &'a [AdvSet] => {
                kind: counted_items,
                count: u8 => 1,
                item: AdvSet => 4,
                max_items: 63,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetAdvertisingData(GAP_ADV_SET_ADV_DATA) {
        Params<'a> = {
            adv_handle: AdvertisingHandle => 1,
            operation: u8 => 1,
            fragment_preference: bool => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetScanResponseData(GAP_ADV_SET_SCAN_RESPONSE_DATA) {
        Params<'a> = {
            adv_handle: AdvertisingHandle => 1,
            operation: u8 => 1,
            fragment_preference: bool => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvRemoveSet(GAP_ADV_REMOVE_SET) {
        Params = {
            handle: AdvertisingHandle => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvClearSets(GAP_ADV_CLEAR_SETS) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetRandomAddress(GAP_ADV_SET_RANDOM_ADDRESS) {
        Params = {
            handle: AdvertisingHandle => 1,
            address: BdAddr => 6,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPairingRequestReply(GAP_PAIRING_REQUEST_REPLY) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            accept: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetPeriodicParameters(GAP_ADV_SET_PERIODIC_PARAMETERS) {
        Params = {
            advertising_handle: AdvertisingHandle => 1,
            periodic_adv_interval_min: u16 => 2,
            periodic_adv_interval_max: u16 => 2,
            periodic_adv_properties: u16 => 2,
            num_subevents: u8 => 1,
            subevent_interval: u8 => 1,
            response_slot_delay: u8 => 1,
            response_slot_spacing: u8 => 1,
            num_response_slots: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetPeriodicData(GAP_ADV_SET_PERIODIC_DATA) {
        Params<'a> = {
            advertising_handle: AdvertisingHandle => 1,
            operation: u8 => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 252,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetPeriodicEnable(GAP_ADV_SET_PERIODIC_ENABLE) {
        Params = {
            enable: u8 => 1,
            handle: AdvertisingHandle => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetConfigurationV2(GAP_ADV_SET_CONFIGURATION_V2) {
        Params = {
            adv_mode: u8 => 1,
            adv_handle: AdvertisingHandle => 1,
            adv_event_properties: u16 => 2,
            primary_adv_interval_min: u32 => 4,
            primary_adv_interval_max: u32 => 4,
            primary_adv_channel_map: u8 => 1,
            own_addr_type: u8 => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: u8 => 1,
            adv_tx_power: u8 => 1,
            primary_adv_phy: u8 => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: u8 => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
            primary_adv_phy_options: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapExtStartScan(GAP_EXT_START_SCAN) {
        Params<'a> = {
            scan_mode: u8 => 1,
            procedure: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: u8 => 1,
            duration: u16 => 2,
            period: u16 => 2,
            scanning_filter_policy: u8 => 1,
            scanning_phys: u8 => 1,
            phy_params: &'a [ExtScanPhyParams] => {
                kind: bitmap_items,
                bitmap: scanning_phys,
                mask: 0x05,
                item: ExtScanPhyParams => 5,
                max_items: 2,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapExtCreateConnection(GAP_EXT_CREATE_CONNECTION) {
        Params<'a> = {
            initiating_mode: u8 => 1,
            procedure: u8 => 1,
            own_address_type: u8 => 1,
            peer_address_type: u8 => 1,
            peer_address: BdAddr => 6,
            advertising_handle: u8 => 1,
            subevent: u8 => 1,
            initiator_filter_policy: u8 => 1,
            initiating_phys: u8 => 1,
            phy_params: &'a [[u8; 16]] => {
                kind: bitmap_items,
                bitmap: initiating_phys,
                mask: 0x07,
                item: [u8; 16] => 16,
                max_items: 3,
            },
        };
        Completion = CommandStatus;
    }
}

impl<T> GapCommands for T
where
    T: ControllerCmdSync<GapSetNonDiscoverable>
        + for<'t> ControllerCmdAsync<GapSetLimitedDiscoverable<'t>>
        + for<'t> ControllerCmdSync<GapSetDiscoverable<'t>>
        + ControllerCmdSync<GapSetDirectConnectable>
        + ControllerCmdSync<GapSetIoCapability>
        + ControllerCmdSync<GapSetAuthenticationRequirement>
        + ControllerCmdSync<GapSetAuthorizationRequirement>
        + ControllerCmdSync<GapPassKeyResponse>
        + ControllerCmdSync<GapAuthorizationResponse>
        + ControllerCmdSync<CmdGapInit>
        + ControllerCmdSync<GapSetNonConnectable>
        + ControllerCmdSync<GapSetUnidirectedConnectable>
        + ControllerCmdAsync<GapPeripheralSecurityRequest>
        + for<'t> ControllerCmdSync<GapUpdateAdvertisingData<'t>>
        + ControllerCmdSync<GapGetSecurityLevel>
        + ControllerCmdSync<GapSetEventMask>
        + ControllerCmdSync<GapConfigureWhitelist>
        + ControllerCmdAsync<GapTerminate>
        + ControllerCmdSync<GapClearSecurityDatabase>
        + ControllerCmdSync<GapAllowRebond>
        + ControllerCmdAsync<GapStartLimitedDiscoveryProcedure>
        + ControllerCmdAsync<GapStartGeneralDiscoveryProcedure>
        + for<'t> ControllerCmdAsync<GapStartAutoConnectionEstablishmentProcedure<'t>>
        + ControllerCmdAsync<GapStartGeneralConnectionEstablishmentProcedure>
        + for<'t> ControllerCmdAsync<GapStartSelectiveConnectionEstablishmentProcedure<'t>>
        + ControllerCmdAsync<GapCreateConnection>
        + ControllerCmdSync<GapTerminateProcedure>
        + ControllerCmdSync<CmdGapResolvePrivateAddress>
        + for<'t> ControllerCmdSync<GapSetBroadcastMode<'t>>
        + ControllerCmdAsync<GapStartObservationProcedure>
        + ControllerCmdSync<GapGetBondedDevices>
        + ControllerCmdSync<GapIsDeviceBonded>
        + ControllerCmdSync<GapConfirmNumericComparisonValue>
        + ControllerCmdAsync<GapStartConnectionUpdate>
        + ControllerCmdAsync<GapSendPairingRequest>
        + ControllerCmdSync<GapPasskeyInput>
        + ControllerCmdSync<GapGetOobData>
        + ControllerCmdSync<GapSetOobData>
        + for<'t> ControllerCmdSync<GapAddDevicesToResolvingList<'t>>
        + ControllerCmdSync<GapRemoveBondedDevice>
        + ControllerCmdSync<GapAdditionalBeaconStart>
        + ControllerCmdSync<GapAdditionalBeaconStop>
        + for<'t> ControllerCmdSync<GapAdditionalBeaconSetData<'t>>
        + for<'t> ControllerCmdSync<GapAdvSetConfig<'t>>
        + for<'t> ControllerCmdSync<GapAdvSetEnable<'t>>
        + for<'t> ControllerCmdSync<GapAdvSetAdvertisingData<'t>>
        + for<'t> ControllerCmdSync<GapAdvSetScanResponseData<'t>>
        + ControllerCmdSync<GapAdvRemoveSet>
        + for<'t> ControllerCmdSync<GapAddDevicesToList<'t>>
        + ControllerCmdSync<GapAdvClearSets>
        + ControllerCmdSync<GapAdvSetRandomAddress>
        + ControllerCmdSync<GapDeleteAdType>
        + ControllerCmdSync<GapPairingRequestReply>
        + ControllerCmdSync<GapAdvSetPeriodicParameters>
        + for<'t> ControllerCmdSync<GapAdvSetPeriodicData<'t>>
        + ControllerCmdSync<GapAdvSetPeriodicEnable>
        + ControllerCmdSync<GapAdvSetConfigurationV2>
        + for<'t> ControllerCmdAsync<GapExtStartScan<'t>>
        + for<'t> ControllerCmdAsync<GapExtCreateConnection<'t>>,
{
    async fn gap_set_nondiscoverable(&self) -> Result<(), Error> {
        GapSetNonDiscoverable::new()
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn set_limited_discoverable(
        &self,
        params: &DiscoverableParameters<'_, '_>,
    ) -> Result<(), Error> {
        params.validate()?;
        let mut local_name = [0; 255];
        let local_name = encode_local_name(&params.local_name, &mut local_name)?;
        let advertising_interval = params
            .advertising_interval
            .unwrap_or((Duration::ZERO, Duration::ZERO));
        GapSetLimitedDiscoverable::try_new(
            params.advertising_type as u8,
            to_connection_length_value(advertising_interval.0),
            to_connection_length_value(advertising_interval.1),
            params.address_type as u8,
            params.filter_policy as u8,
            local_name,
            params.advertising_data,
            params.conn_interval.0.map_or(0, to_conn_interval_value),
            params.conn_interval.1.map_or(0, to_conn_interval_value),
        )
        .map_err(|_| Error::IoError)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn set_discoverable(&self, params: &DiscoverableParameters<'_, '_>) -> Result<(), Error> {
        params.validate()?;
        let mut local_name = [0; 255];
        let local_name = encode_local_name(&params.local_name, &mut local_name)?;
        let advertising_interval = params
            .advertising_interval
            .unwrap_or((Duration::ZERO, Duration::ZERO));
        GapSetDiscoverable::try_new(
            params.advertising_type as u8,
            to_connection_length_value(advertising_interval.0),
            to_connection_length_value(advertising_interval.1),
            params.address_type as u8,
            params.filter_policy as u8,
            local_name,
            params.advertising_data,
            params.conn_interval.0.map_or(0, to_conn_interval_value),
            params.conn_interval.1.map_or(0, to_conn_interval_value),
        )
        .map_err(|_| Error::IoError)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn set_direct_connectable(
        &self,
        params: &DirectConnectableParameters,
    ) -> Result<(), Error> {
        params.validate()?;
        GapSetDirectConnectable::new(
            params.own_address_type as u8,
            params.advertising_type as u8,
            params.initiator_address,
            to_connection_length_value(params.advertising_interval.0),
            to_connection_length_value(params.advertising_interval.1),
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn set_io_capability(&self, capability: IoCapability) -> Result<(), Error> {
        GapSetIoCapability::new(capability)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn set_authentication_requirement(
        &self,
        requirements: &AuthenticationRequirements,
    ) -> Result<(), Error> {
        requirements.validate()?;
        let (pass_key_required, fixed_pin) = match requirements.fixed_pin {
            Pin::Requested => (true, 0),
            Pin::Fixed(pin) => (false, pin),
        };
        GapSetAuthenticationRequirement::new(
            requirements.bonding_required,
            requirements.mitm_protection_required,
            requirements.secure_connection_support as u8,
            requirements.keypress_notification_support,
            requirements.encryption_key_size_range.0,
            requirements.encryption_key_size_range.1,
            pass_key_required,
            fixed_pin,
            requirements.identity_address_type as u8,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn set_authorization_requirement(
        &self,
        conn_handle: crate::ConnectionHandle,
        authorization_required: bool,
    ) -> Result<(), Error> {
        GapSetAuthorizationRequirement::new(conn_handle, authorization_required)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn pass_key_response(
        &self,
        conn_handle: crate::ConnectionHandle,
        pin: u32,
    ) -> Result<(), Error> {
        if pin > 999_999 {
            return Err(Error::BadFixedPin(pin));
        }

        GapPassKeyResponse::new(conn_handle, pin)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn authorization_response(
        &self,
        conn_handle: crate::ConnectionHandle,
        authorization: Authorization,
    ) -> Result<(), Error> {
        GapAuthorizationResponse::new(conn_handle, authorization as u8)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn init(
        &self,
        role: Role,
        privacy_enabled: bool,
        dev_name_characteristic_len: u8,
    ) -> Result<GapInit, Error> {
        CmdGapInit::new(role, privacy_enabled, dev_name_characteristic_len)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn set_nonconnectable(
        &self,
        advertising_type: AdvertisingType,
        address_type: AddressType,
    ) -> Result<(), Error> {
        match advertising_type {
            AdvertisingType::ScannableUndirected | AdvertisingType::NonConnectableUndirected => (),
            _ => {
                return Err(Error::BadAdvertisingType(advertising_type));
            }
        }

        GapSetNonConnectable::new(advertising_type as u8, address_type as u8)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn set_undirected_connectable(
        &self,
        params: &UndirectedConnectableParameters,
    ) -> Result<(), Error> {
        params.validate()?;
        GapSetUnidirectedConnectable::new(
            to_connection_length_value(params.advertising_interval.0),
            to_connection_length_value(params.advertising_interval.1),
            params.own_address_type as u8,
            params.filter_policy as u8,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn peripheral_security_request(
        &self,
        conn_handle: &ConnectionHandle,
    ) -> Result<(), Error> {
        GapPeripheralSecurityRequest::new(*conn_handle)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn update_advertising_data(&self, data: &[u8]) -> Result<(), Error> {
        GapUpdateAdvertisingData::try_new(data)
            .map_err(|error| Error::BadAdvertisingDataLength(error.actual()))?
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn delete_ad_type(&self, ad_type: AdvertisingDataType) -> Result<(), Error> {
        GapDeleteAdType::new(ad_type as u8)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_security_level(
        &self,
        conn_handle: &ConnectionHandle,
    ) -> Result<GapSecurityLevel, Error> {
        let response = GapGetSecurityLevel::new(*conn_handle)
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(GapSecurityLevel {
            security_mode: response.security_mode,
            security_level: response.security_level,
        })
    }

    async fn set_event_mask(&self, flags: EventFlags) -> Result<(), Error> {
        GapSetEventMask::new(flags.bits())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn configure_white_list(&self) -> Result<(), Error> {
        GapConfigureWhitelist::new()
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn terminate(
        &self,
        conn_handle: crate::ConnectionHandle,
        reason: crate::Status,
    ) -> Result<(), Error> {
        match reason {
            crate::Status::AuthFailure
            | crate::Status::RemoteTerminationByUser
            | crate::Status::RemoteTerminationLowResources
            | crate::Status::RemoteTerminationPowerOff
            | crate::Status::UnsupportedRemoteFeature
            | crate::Status::PairingWithUnitKeyNotSupported
            | crate::Status::UnacceptableConnectionParameters => (),
            _ => {
                return Err(Error::BadTerminationReason(reason));
            }
        }

        GapTerminate::new(conn_handle, reason.into())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn clear_security_database(&self) -> Result<(), Error> {
        GapClearSecurityDatabase::new()
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn allow_rebond(&self, conn_handle: crate::ConnectionHandle) -> Result<(), Error> {
        GapAllowRebond::new(conn_handle)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn start_limited_discovery_procedure(
        &self,
        params: &DiscoveryProcedureParameters,
    ) -> Result<(), Error> {
        GapStartLimitedDiscoveryProcedure::new(
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.own_address_type as u8,
            params.filter_duplicates,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn start_general_discovery_procedure(
        &self,
        params: &DiscoveryProcedureParameters,
    ) -> Result<(), Error> {
        GapStartGeneralDiscoveryProcedure::new(
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.own_address_type as u8,
            params.filter_duplicates,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn start_auto_connection_establishment_procedure(
        &self,
        params: &AutoConnectionEstablishmentParameters<'_>,
    ) -> Result<(), Error> {
        params.validate()?;
        let conn_interval = params.conn_interval.interval();
        let expected_connection_length = params.expected_connection_length.range;
        GapStartAutoConnectionEstablishmentProcedure::try_new(
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.own_address_type as u8,
            to_conn_interval_value(conn_interval.0),
            to_conn_interval_value(conn_interval.1),
            params.conn_interval.conn_latency(),
            to_supervision_timeout_value(params.conn_interval.supervision_timeout()),
            to_connection_length_value(expected_connection_length.0),
            to_connection_length_value(expected_connection_length.1),
            params.white_list,
        )
        .map_err(|_| Error::WhiteListTooLong)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn start_general_connection_establishment_procedure(
        &self,
        params: &GeneralConnectionEstablishmentParameters,
    ) -> Result<(), Error> {
        GapStartGeneralConnectionEstablishmentProcedure::new(
            params.scan_type as u8,
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.filter_policy as u8,
            params.own_address_type as u8,
            params.filter_duplicates,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn start_selective_connection_establishment_procedure(
        &self,
        params: &SelectiveConnectionEstablishmentParameters<'_>,
    ) -> Result<(), Error> {
        params.validate()?;
        GapStartSelectiveConnectionEstablishmentProcedure::try_new(
            params.scan_type as u8,
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.own_address_type as u8,
            params.filter_policy as u8,
            params.filter_duplicates,
            params.white_list,
        )
        .map_err(|_| Error::WhiteListTooLong)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn create_connection(&self, params: &ConnectionParameters) -> Result<(), Error> {
        let conn_interval = params.conn_interval.interval();
        let expected_connection_length = params.expected_connection_length.range;
        GapCreateConnection::new(
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.peer_address,
            params.own_address_type as u8,
            to_conn_interval_value(conn_interval.0),
            to_conn_interval_value(conn_interval.1),
            params.conn_interval.conn_latency(),
            to_supervision_timeout_value(params.conn_interval.supervision_timeout()),
            to_connection_length_value(expected_connection_length.0),
            to_connection_length_value(expected_connection_length.1),
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn terminate_gap_procedure(&self, procedure: Procedure) -> Result<(), Error> {
        if procedure.is_empty() {
            return Err(Error::NoProcedure);
        }

        GapTerminateProcedure::new(procedure.bits())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn start_connection_update(
        &self,
        params: &ConnectionUpdateParameters,
    ) -> Result<(), Error> {
        let conn_interval = params.conn_interval.interval();
        let expected_connection_length = params.expected_connection_length.range;
        GapStartConnectionUpdate::new(
            params.conn_handle,
            to_conn_interval_value(conn_interval.0),
            to_conn_interval_value(conn_interval.1),
            params.conn_interval.conn_latency(),
            to_supervision_timeout_value(params.conn_interval.supervision_timeout()),
            to_connection_length_value(expected_connection_length.0),
            to_connection_length_value(expected_connection_length.1),
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn send_pairing_request(&self, params: &PairingRequest) -> Result<(), Error> {
        GapSendPairingRequest::new(params.conn_handle, params.force_rebond)
            .exec(self)
            .await
            .map_err(Error::from)
    }
    async fn resolve_private_address(
        &self,
        addr: crate::BdAddr,
    ) -> Result<GapResolvePrivateAddress, Error> {
        let response = CmdGapResolvePrivateAddress::new(addr)
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(GapResolvePrivateAddress {
            bd_addr: Some(response.address),
        })
    }

    async fn set_broadcast_mode(
        &self,
        params: &BroadcastModeParameters<'_, '_>,
    ) -> Result<(), Error> {
        params.validate()?;
        GapSetBroadcastMode::try_new(
            to_connection_length_value(params.advertising_interval.interval.0),
            to_connection_length_value(params.advertising_interval.interval.1),
            params.advertising_interval.advertising_type() as u8,
            params.own_address_type as u8,
            params.advertising_data,
            params.white_list,
        )
        .map_err(|_| Error::WhiteListTooLong)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn start_observation_procedure(
        &self,
        params: &ObservationProcedureParameters,
    ) -> Result<(), Error> {
        GapStartObservationProcedure::new(
            to_connection_length_value(params.scan_window.interval()),
            to_connection_length_value(params.scan_window.window()),
            params.scan_type as u8,
            params.own_address_type as u8,
            params.filter_duplicates,
            params.filter_policy as u8,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn get_bonded_devices(&self) -> Result<GapBondedDevices, Error> {
        GapGetBondedDevices::new()
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn is_device_bonded(&self, addr: crate::host::PeerAddrType) -> Result<(), Error> {
        GapIsDeviceBonded::new(addr)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn numeric_comparison_value_confirm_yes_no(
        &self,
        params: &NumericComparisonValueConfirmYesNoParameters,
    ) -> Result<(), Error> {
        GapConfirmNumericComparisonValue::new(params.conn_handle, params.confirm_yes_no)
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn passkey_input(
        &self,
        conn_handle: ConnectionHandle,
        input_type: InputType,
    ) -> Result<(), Error> {
        GapPasskeyInput::new(conn_handle, input_type as u8)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_oob_data(&self, oob_data_type: OobDataType) -> Result<[u8; 26], Error> {
        let response = GapGetOobData::new(oob_data_type as u8)
            .exec(self)
            .await
            .map_err(Error::from)?;
        let mut data = [0; 26];
        data[1] = response.address_type;
        data[2..8].copy_from_slice(&response.address.0);
        data[8] = response.oob_data_type;
        data[9] = response.oob_data_len;
        data[10..].copy_from_slice(&response.oob_data);
        Ok(data)
    }

    async fn set_oob_data(&self, params: &SetOobDataParameters) -> Result<(), Error> {
        GapSetOobData::new(
            params.device_type as u8,
            params.address,
            params.oob_data_type as u8,
            params.oob_data.len() as u8,
            params.oob_data,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn add_devices_to_resolving_list(
        &self,
        whitelist_identities: &[PeerAddrType],
        clear_resolving_list: bool,
    ) -> Result<(), Error> {
        GapAddDevicesToResolvingList::try_new(whitelist_identities, clear_resolving_list)
            .map_err(|_| Error::WhiteListTooLong)?
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn remove_bonded_device(&self, address: BdAddrType) -> Result<(), Error> {
        GapRemoveBondedDevice::new(address)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn add_devices_to_list(
        &self,
        list_entries: &[BdAddrType],
        mode: AddDeviceToListMode,
    ) -> Result<(), Error> {
        GapAddDevicesToList::try_new(list_entries, mode as u8)
            .map_err(|_| Error::WhiteListTooLong)?
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn additional_beacon_start(
        &self,
        params: &AdditonalBeaconStartParameters,
    ) -> Result<(), Error> {
        params.validate()?;
        GapAdditionalBeaconStart::new(
            to_connection_length_value(params.advertising_interval.0),
            to_connection_length_value(params.advertising_interval.1),
            params.advertising_channel_map.bits(),
            params.own_address_type,
            params.pa_level,
        )
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn additional_beacon_stop(&self) -> Result<(), Error> {
        GapAdditionalBeaconStop::new()
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn additonal_beacon_set_data(&self, advertising_data: &[u8]) -> Result<(), Error> {
        GapAdditionalBeaconSetData::try_new(advertising_data)
            .map_err(|_| Error::BadAdvertisingDataLength(advertising_data.len()))?
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn adv_set_config(&self, params: &AdvSetConfig) -> Result<(), Error> {
        GapAdvSetConfig::try_new(
            params.adv_mode.bits(),
            params.adv_handle,
            params.adv_event_properties.bits(),
            &params.adv_interval,
            params.primary_adv_channel_map.bits(),
            params.own_addr_type as u8,
            params.peer_addr,
            params.adv_filter_policy as u8,
            params.adv_tx_power,
            params.secondary_adv_max_skip,
            params.secondary_adv_phy as u8,
            params.adv_sid,
            params.scan_req_notification_enable,
        )
        .map_err(|_| Error::IoError)?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn adv_set_enable<'a>(&self, params: &AdvSetEnable<'a>) -> Result<(), Error> {
        if usize::from(params.num_sets) != params.adv_set.len() {
            return Err(Error::IoError);
        }
        GapAdvSetEnable::try_new(params.enable, params.adv_set)
            .map_err(|_| Error::IoError)?
            .exec(self)
            .await
            .map_err(Error::from)
    }

    async fn adv_set_advertising_data(
        &self,
        params: &AdvSetAdvertisingData<'_>,
    ) -> Result<(), Error> {
        GapAdvSetAdvertisingData::try_new(
            params.adv_handle,
            params.operation as u8,
            !params.fragment,
            params.data,
        )
        .map_err(|_| Error::BadAdvertisingDataLength(params.data.len()))?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn adv_set_scan_response_data(
        &self,
        params: &AdvSetAdvertisingData<'_>,
    ) -> Result<(), Error> {
        GapAdvSetScanResponseData::try_new(
            params.adv_handle,
            params.operation as u8,
            !params.fragment,
            params.data,
        )
        .map_err(|_| Error::BadAdvertisingDataLength(params.data.len()))?
        .exec(self)
        .await
        .map_err(Error::from)
    }

    async fn adv_remove_set(&self, handle: AdvertisingHandle) -> Result<(), Error> {
        GapAdvRemoveSet::new(handle)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn adv_clear_sets(&self) -> Result<(), Error> {
        GapAdvClearSets::new()
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn adv_set_random_address(
        &self,
        handle: AdvertisingHandle,
        addr: BdAddr,
    ) -> Result<(), Error> {
        GapAdvSetRandomAddress::new(handle, addr)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn pairing_request_reply(
        &self,
        conn_handle: crate::ConnectionHandle,
        accept: bool,
    ) -> Result<(), Error> {
        GapPairingRequestReply::new(conn_handle, accept)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn adv_set_periodic_parameters(
        &self,
        params: &AdvSetPeriodicParameters,
    ) -> Result<(), Error> {
        GapAdvSetPeriodicParameters::new(
            params.advertising_handle,
            params.periodic_adv_interval_min,
            params.periodic_adv_interval_max,
            params.periodic_adv_properties,
            params.num_subevents,
            params.subevent_interval,
            params.response_slot_delay,
            params.response_slot_spacing,
            params.num_response_slots,
        )
        .exec(self)
        .await
        .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn adv_set_periodic_data<'a>(
        &self,
        params: &AdvSetPeriodicData<'a>,
    ) -> Result<(), Error> {
        GapAdvSetPeriodicData::try_new(
            params.advertising_handle,
            params.operation as u8,
            params.data,
        )
        .map_err(|_| Error::BadAdvertisingDataLength(params.data.len()))?
        .exec(self)
        .await
        .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn adv_set_periodic_enable(
        &self,
        enable: u8,
        handle: AdvertisingHandle,
    ) -> Result<(), Error> {
        GapAdvSetPeriodicEnable::new(enable, handle)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn adv_set_configuration_v2(&self, params: &AdvSetConfigV2) -> Result<(), Error> {
        GapAdvSetConfigurationV2::new(
            params.adv_mode.bits(),
            params.adv_handle,
            params.adv_event_properties.bits(),
            params.primary_adv_interval_min,
            params.primary_adv_interval_max,
            params.primary_adv_channel_map.bits(),
            params.own_addr_type as u8,
            params.peer_addr,
            params.adv_filter_policy as u8,
            params.adv_tx_power,
            params.primary_adv_phy as u8,
            params.secondary_adv_max_skip,
            params.secondary_adv_phy as u8,
            params.adv_sid,
            params.scan_req_notification_enable,
            params.primary_adv_phy_options,
        )
        .exec(self)
        .await
        .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn ext_start_scan(&self, params: &ExtStartScanParams) -> Result<(), Error> {
        let phy_params = params.phy_params.get(..params.num_phys).ok_or_else(|| {
            Error::BadExtendedScanParameters(HciLengthError::new(
                params.num_phys,
                0,
                params.phy_params.len(),
            ))
        })?;

        GapExtStartScan::try_new(
            params.scan_mode,
            params.procedure,
            params.own_address_type,
            params.filter_duplicates,
            params.duration,
            params.period,
            params.scanning_filter_policy,
            params.scanning_phys,
            phy_params,
        )
        .map_err(Error::BadExtendedScanParameters)?
        .exec(self)
        .await
        .map_err(|e| e.into())
    }

    #[cfg(after_fw_0_17_1)]
    async fn ext_create_connection(&self, params: &ExtCreateConnectionParams) -> Result<(), Error> {
        let phy_params = params.phy_params.get(..params.num_phys).ok_or_else(|| {
            Error::BadExtendedScanParameters(HciLengthError::new(
                params.num_phys,
                0,
                params.phy_params.len(),
            ))
        })?;
        let mut encoded_phy_params = [[0; 16]; 3];
        for (encoded, phy) in encoded_phy_params.iter_mut().zip(phy_params) {
            LittleEndian::write_u16(&mut encoded[0..2], phy.scan_interval);
            LittleEndian::write_u16(&mut encoded[2..4], phy.scan_window);
            LittleEndian::write_u16(&mut encoded[4..6], phy.conn_interval_min);
            LittleEndian::write_u16(&mut encoded[6..8], phy.conn_interval_max);
            LittleEndian::write_u16(&mut encoded[8..10], phy.conn_latency);
            LittleEndian::write_u16(&mut encoded[10..12], phy.supervision_timeout);
            LittleEndian::write_u16(&mut encoded[12..14], phy.min_ce_length);
            LittleEndian::write_u16(&mut encoded[14..16], phy.max_ce_length);
        }
        GapExtCreateConnection::try_new(
            params.initiating_mode,
            params.procedure,
            params.own_address_type,
            params.peer_address_type,
            params.peer_address,
            params.advertising_handle,
            params.subevent,
            params.initiator_filter_policy,
            params.initiating_phys,
            &encoded_phy_params[..phy_params.len()],
        )
        .map_err(Error::BadExtendedScanParameters)?
        .exec(self)
        .await
        .map_err(|e| e.into())
    }
}

/// Potential errors from parameter validation.
///
/// Before some commands are sent to the controller, the parameters are validated. This type
/// enumerates the potential validation errors. Must be specialized on the types of communication
/// errors.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// For the [GAP Set Limited Discoverable](GapCommands::set_limited_discoverable) and
    /// [GAP Set Discoverable](GapCommands::set_discoverable) commands, the connection
    /// interval is inverted (the min is greater than the max).  Return the provided min as the
    /// first element, max as the second.
    BadConnectionInterval(Duration, Duration),

    /// For the [GAP Set Limited Discoverable](GapCommands::set_limited_discoverable) and
    /// [GAP Set Broadcast Mode](GapCommands::set_broadcast_mode) commands, the advertising
    /// type is disallowed.  Returns the invalid advertising type.
    BadAdvertisingType(crate::types::AdvertisingType),

    /// For the [GAP Set Limited Discoverable](GapCommands::set_limited_discoverable)
    /// command, the advertising interval is inverted (that is, the max is less than the
    /// min). Includes the provided range.
    BadAdvertisingInterval(Duration, Duration),

    /// For the [GAP Set Authentication Requirement](GapCommands::set_authentication_requirement)
    /// command, the encryption key size range is inverted (the max is less than the min). Includes the provided range.
    BadEncryptionKeySizeRange(u8, u8),

    /// For the [GAP Set Authentication Requirement](GapCommands::set_authentication_requirement)
    /// command, the address type must be either Public or Random
    BadAddressType(AddressType),

    BadPowerAmplifierLevel(u8),

    /// For the [GAP Set Authentication Requirement](GapCommands::set_authentication_requirement) and
    /// [GAP Pass Key Response](GapCommands::pass_key_response) commands, the provided fixed pin is out of
    /// range (must be less than or equal to 999999).  Includes the provided PIN.
    BadFixedPin(u32),

    /// For the [GAP Set Undirected Connectable](GapCommands::set_undirected_connectable) command, the
    /// advertising filter policy is not one of the allowed values. Only
    /// [AllowConnectionAndScan](crate::host::AdvertisingFilterPolicy::AllowConnectionAndScan) and
    /// [WhiteListConnectionAndScan](crate::host::AdvertisingFilterPolicy::WhiteListConnectionAndScan) are
    /// allowed.
    BadAdvertisingFilterPolicy(crate::host::AdvertisingFilterPolicy),

    /// For the [GAP Update Advertising Data](GapCommands::update_advertising_data) and
    /// [GAP Set Broadcast Mode](GapCommands::set_broadcast_mode) commands, the advertising data
    /// is too long. It must be 31 bytes or less. The length of the provided data is returned.
    BadAdvertisingDataLength(usize),

    /// For extended scanning, the PHY bitmap selects an unsupported bit, or
    /// the number of per-PHY records differs from the selected-bit count.
    #[cfg(after_fw_0_17_1)]
    BadExtendedScanParameters(HciLengthError),

    /// For the [GAP Terminate](GapCommands::terminate) command, the termination reason was
    /// not one of the allowed reason. The reason is returned.
    BadTerminationReason(crate::Status),

    /// For the [GAP Start Auto Connection Establishment](GapCommands::start_auto_connection_establishment_procedure) or
    /// [GAP Start Selective Connection Establishment](GapCommands::start_selective_connection_establishment_procedure) commands, the
    /// provided [white list](AutoConnectionEstablishmentParameters::white_list) has more than 33
    /// or 35 entries, respectively, which would cause the command to be longer than 255 bytes.
    ///
    /// For the [GAP Set Broadcast Mode](GapCommands::set_broadcast_mode), the provided
    /// [white list](BroadcastModeParameters::white_list) the maximum number of entries ranges
    /// from 31 to 35, depending on the length of the advertising data.
    WhiteListTooLong,

    /// For the [GAP Terminate Procedure](GapCommands::terminate_gap_procedure) command, the
    /// provided bitfield had no bits set.
    NoProcedure,

    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
    }
}

fn to_conn_interval_value(d: Duration) -> u16 {
    // Connection interval value: T = N * 1.25 ms
    // We have T, we need to return N.
    // N = T / 1.25 ms
    //   = 4 * T / 5 ms
    let millis = (d.as_secs() * 1000) as u32 + d.subsec_millis();
    (4 * millis / 5) as u16
}

fn to_supervision_timeout_value(d: Duration) -> u16 {
    (100 * d.as_secs() as u32 + d.subsec_millis() / 10) as u16
}

fn to_connection_length_value(d: Duration) -> u16 {
    // Connection interval value: T = N * 0.625 ms
    // We have T, we need to return N.
    // N = T / 0.625 ms
    //   = T / 625 us
    // 1600 = 1_000_000 / 625
    (1600 * d.as_secs() as u32 + (d.subsec_micros() / 625)) as u16
}

fn encode_local_name<'a>(
    local_name: &Option<LocalName<'_>>,
    bytes: &'a mut [u8; 255],
) -> Result<&'a [u8], Error> {
    let (ad_type, name) = match local_name {
        None => return Ok(&bytes[..0]),
        Some(LocalName::Shortened(name)) => (0x08, *name),
        Some(LocalName::Complete(name)) => (0x09, *name),
    };
    if name.len() > 241 {
        return Err(Error::IoError);
    }
    bytes[0] = ad_type;
    bytes[1..1 + name.len()].copy_from_slice(name);
    Ok(&bytes[..1 + name.len()])
}

impl crate::vendor::command::HciEncodeField<8> for ExtendedAdvertisingInterval {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

impl crate::vendor::command::HciEncodeField<7> for PeerAddrType {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 7];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 7];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

impl crate::vendor::command::HciEncodeField<4> for AdvSet {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

/// Parameters for the
/// [`set_limited_discoverable`](GapCommands::set_limited_discoverable) and
/// [`set_discoverable`](GapCommands::set_discoverable) commands.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DiscoverableParameters<'a, 'b> {
    /// Advertising method for the device.
    ///
    /// Must be
    /// [ConnectableUndirected](crate::host::AdvertisingType::ConnectableUndirected),
    /// [ScannableUndirected](crate::host::AdvertisingType::ScannableUndirected), or
    /// [NonConnectableUndirected](crate::host::AdvertisingType::NonConnectableUndirected).
    pub advertising_type: AdvertisingType,

    /// Range of advertising for non-directed advertising.
    ///
    /// If not provided, the GAP will use default values (1.28 seconds).
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: Option<(Duration, Duration)>,

    /// Address type for this device.
    pub address_type: OwnAddressType,

    /// Filter policy for this device.
    pub filter_policy: AdvertisingFilterPolicy,

    /// Name of the device.
    pub local_name: Option<LocalName<'a>>,

    /// Service UUID list as defined in the Bluetooth spec, v4.1, Vol 3, Part C, Section 11.
    ///
    /// Must be 31 bytes or fewer.
    pub advertising_data: &'b [u8],

    /// Expected length of the connection to the peripheral.
    pub conn_interval: (Option<Duration>, Option<Duration>),
}

impl<'a, 'b> DiscoverableParameters<'a, 'b> {
    fn validate(&self) -> Result<(), Error> {
        if self.advertising_data.len() > 31 {
            return Err(Error::BadAdvertisingDataLength(self.advertising_data.len()));
        }

        match self.advertising_type {
            AdvertisingType::ConnectableUndirected
            | AdvertisingType::ScannableUndirected
            | AdvertisingType::NonConnectableUndirected => (),
            _ => return Err(Error::BadAdvertisingType(self.advertising_type)),
        }

        if let Some(interval) = self.advertising_interval
            && interval.0 > interval.1
        {
            return Err(Error::BadAdvertisingInterval(interval.0, interval.1));
        }

        if let (Some(min), Some(max)) = self.conn_interval
            && min > max
        {
            return Err(Error::BadConnectionInterval(min, max));
        }

        Ok(())
    }
}

/// Allowed types for the local name.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LocalName<'a> {
    /// The shortened local name.
    Shortened(&'a [u8]),

    /// The complete local name.
    Complete(&'a [u8]),
}

/// Parameters for the
/// [`set_undirected_connectable`](GapCommands::set_undirected_connectable) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UndirectedConnectableParameters {
    /// Range of advertising interval for advertising.
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: (Duration, Duration),

    /// Address type of this device.
    pub own_address_type: OwnAddressType,

    /// filter policy for this device
    pub filter_policy: AdvertisingFilterPolicy,
}

impl UndirectedConnectableParameters {
    fn validate(&self) -> Result<(), Error> {
        const MIN_DURATION: Duration = Duration::from_millis(20);
        const MAX_DURATION: Duration = Duration::from_millis(10240);

        match self.filter_policy {
            AdvertisingFilterPolicy::AllowConnectionAndScan
            | AdvertisingFilterPolicy::WhiteListConnectionAndScan => {}
            _ => return Err(Error::BadAdvertisingFilterPolicy(self.filter_policy)),
        }

        if self.advertising_interval.0 < MIN_DURATION
            || self.advertising_interval.1 > MAX_DURATION
            || self.advertising_interval.0 > self.advertising_interval.1
        {
            return Err(Error::BadAdvertisingInterval(
                self.advertising_interval.0,
                self.advertising_interval.1,
            ));
        }

        Ok(())
    }
}

/// Parameters for the
/// [`set_direct_connectable`](GapCommands::set_direct_connectable) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DirectConnectableParameters {
    /// Address type of this device.
    pub own_address_type: OwnAddressType,

    /// Advertising method for the device.
    ///
    /// Must be
    /// [ConnectableDirectedHighDutyCycle](crate::host::AdvertisingType::ConnectableDirectedHighDutyCycle),
    /// or
    /// [ConnectableDirectedLowDutyCycle](crate::host::AdvertisingType::ConnectableDirectedLowDutyCycle).
    pub advertising_type: AdvertisingType,

    /// Initiator's Bluetooth address.
    pub initiator_address: BdAddrType,

    /// Range of advertising interval for advertising.
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: (Duration, Duration),
}

impl DirectConnectableParameters {
    fn validate(&self) -> Result<(), Error> {
        const MIN_DURATION: Duration = Duration::from_millis(20);
        const MAX_DURATION: Duration = Duration::from_millis(10240);

        match self.advertising_type {
            AdvertisingType::ConnectableDirectedHighDutyCycle
            | AdvertisingType::ConnectableDirectedLowDutyCycle => (),
            _ => return Err(Error::BadAdvertisingType(self.advertising_type)),
        }

        if self.advertising_interval.0 < MIN_DURATION
            || self.advertising_interval.1 > MAX_DURATION
            || self.advertising_interval.0 > self.advertising_interval.1
        {
            return Err(Error::BadAdvertisingInterval(
                self.advertising_interval.0,
                self.advertising_interval.1,
            ));
        }

        Ok(())
    }
}

/// I/O capabilities available for the [GAP Set I/O Capability](GapCommands::set_io_capability) command.
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum IoCapability {
    /// Display Only
    Display = 0x00,
    /// Display yes/no
    DisplayConfirm = 0x01,
    /// Keyboard Only
    Keyboard = 0x02,
    /// No Input, no output
    None = 0x03,
    /// Keyboard display
    KeyboardDisplay = 0x04,
}

impl crate::vendor::command::HciEncodeField<1> for IoCapability {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field(&(*self as u8), writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field_async(
            &(*self as u8),
            writer,
        )
        .await
    }
}

/// Parameters for the [GAP Set Authentication Requirement](GapCommands::set_authentication_requirement) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AuthenticationRequirements {
    /// Is bonding required?
    pub bonding_required: bool,

    /// Is MITM (man-in-the-middle) protection required?
    pub mitm_protection_required: bool,

    /// is secure connection support required
    pub secure_connection_support: SecureConnectionSupport,

    /// is keypress notification support required
    pub keypress_notification_support: bool,

    /// Minimum and maximum size of the encryption key.
    pub encryption_key_size_range: (u8, u8),

    /// Pin to use during the pairing process.
    pub fixed_pin: Pin,

    /// identity address type.
    pub identity_address_type: AddressType,
}

impl AuthenticationRequirements {
    fn validate(&self) -> Result<(), Error> {
        if self.encryption_key_size_range.0 > self.encryption_key_size_range.1 {
            return Err(Error::BadEncryptionKeySizeRange(
                self.encryption_key_size_range.0,
                self.encryption_key_size_range.1,
            ));
        }

        if let Pin::Fixed(pin) = self.fixed_pin
            && pin > 999_999
        {
            return Err(Error::BadFixedPin(pin));
        }

        if self.identity_address_type != AddressType::Public
            && self.identity_address_type != AddressType::Random
        {
            return Err(Error::BadAddressType(self.identity_address_type));
        }

        Ok(())
    }
}

/// Options for out-of-band authentication.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OutOfBandAuthentication {
    /// Out Of Band authentication not enabled
    Disabled,
    /// Out Of Band authentication enabled; includes the OOB data.
    Enabled([u8; 16]),
}

/// Options for [`secure_connection_support`](AuthenticationRequirements)
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SecureConnectionSupport {
    NotSupported = 0x00,
    Optional = 0x01,
    Mandatory = 0x02,
}

/// Options for [`fixed_pin`](AuthenticationRequirements).
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Pin {
    /// Do not use fixed pin during the pairing process.  In this case, GAP will generate a
    /// [GAP Pass Key Request](crate::vendor::event::VendorEvent::GapPassKeyRequest) event to the host.
    Requested,

    /// Use a fixed pin during pairing. The provided value is used as the PIN, and must be 999999 or
    /// less.
    Fixed(u32),
}

/// Options for the [GAP Authorization Response](GapCommands::authorization_response).
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Authorization {
    /// Accept the connection.
    Authorized = 0x01,
    /// Reject the connection.
    Rejected = 0x02,
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Roles for a [GAP service](GapCommands::init).
    pub struct Role: u8 {
        /// Peripheral
        const PERIPHERAL = 0x01;
        /// Broadcaster
        const BROADCASTER = 0x02;
        /// Central Device
        const CENTRAL = 0x04;
        /// Observer
        const OBSERVER = 0x08;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Roles for a [GAP service](GapCommands::init).
    pub struct Role: u8 {
        /// Peripheral
        const PERIPHERAL = 0x01;
        /// Broadcaster
        const BROADCASTER = 0x02;
        /// Central Device
        const CENTRAL = 0x04;
        /// Observer
        const OBSERVER = 0x08;
    }
}

impl crate::vendor::command::HciEncodeField<1> for Role {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field(&self.bits(), writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field_async(
            &self.bits(),
            writer,
        )
        .await
    }
}

/// Indicates the type of address being used in the advertising packets, for the
/// [`set_nonconnectable`](GapCommands::set_nonconnectable).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AddressType {
    /// Public device address.
    Public = 0x00,
    /// Static random device address.
    Random = 0x01,
    /// Controller generates Resolvable Private Address.
    ResolvablePrivate = 0x02,
    /// Controller generates Resolvable Private Address. based on the local IRK from resolving
    /// list.
    NonResolvablePrivate = 0x03,
}

/// Available types of advertising data.
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AdvertisingDataType {
    /// Flags
    Flags = 0x01,
    /// 16-bit service UUID
    Uuid16 = 0x02,
    /// Complete list of 16-bit service UUIDs
    UuidCompleteList16 = 0x03,
    /// 32-bit service UUID
    Uuid32 = 0x04,
    /// Complete list of 32-bit service UUIDs
    UuidCompleteList32 = 0x05,
    /// 128-bit service UUID
    Uuid128 = 0x06,
    /// Complete list of 128-bit service UUIDs.
    UuidCompleteList128 = 0x07,
    /// Shortened local name
    ShortenedLocalName = 0x08,
    /// Complete local name
    CompleteLocalName = 0x09,
    /// Transmitter power level
    TxPowerLevel = 0x0A,
    /// Serurity Manager TK Value
    SecurityManagerTkValue = 0x10,
    /// Serurity Manager out-of-band flags
    SecurityManagerOutOfBandFlags = 0x11,
    /// Connection interval
    PeripheralConnectionInterval = 0x12,
    /// Service solicitation list, 16-bit UUIDs
    SolicitUuidList16 = 0x14,
    /// Service solicitation list, 32-bit UUIDs
    SolicitUuidList32 = 0x15,
    /// Service data
    ServiceData = 0x16,
    /// Manufacturer-specific data
    ManufacturerSpecificData = 0xFF,
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Event types for [GAP Set Event Mask](GapCommands::set_event_mask).
    #[derive(Debug, Clone, Copy)]
    pub struct EventFlags: u16 {
        /// [Limited Discoverable](::event::VendorEvent::GapLimitedDiscoverableTimeout)
        const LIMITED_DISCOVERABLE_TIMEOUT = 0x0001;
        /// [Pairing Complete](::event::VendorEvent::GapPairingComplete)
        const PAIRING_COMPLETE = 0x0002;
        /// [Pass Key Request](::event::VendorEvent::GapPassKeyRequest)
        const PASS_KEY_REQUEST = 0x0004;
        /// [Authorization Request](::event::VendorEvent::GapAuthorizationRequest)
        const AUTHORIZATION_REQUEST = 0x0008;
        /// [Peripheral Security Initiated](::event::VendorEvent::GapPeripheralSecurityInitiated).
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// [Bond Lost](::event::VendorEvent::GapBondLost)
        const BOND_LOST = 0x0020;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Event types for [GAP Set Event Mask](GapCommands::set_event_mask).
    pub struct EventFlags: u16 {
        /// [Limited Discoverable](::event::VendorEvent::GapLimitedDiscoverableTimeout)
        const LIMITED_DISCOVERABLE_TIMEOUT = 0x0001;
        /// [Pairing Complete](::event::VendorEvent::GapPairingComplete)
        const PAIRING_COMPLETE = 0x0002;
        /// [Pass Key Request](::event::VendorEvent::GapPassKeyRequest)
        const PASS_KEY_REQUEST = 0x0004;
        /// [Authorization Request](::event::VendorEvent::GapAuthorizationRequest)
        const AUTHORIZATION_REQUEST = 0x0008;
        /// [Peripheral Security Initiated](::event::VendorEvent::GapPeripheralSecurityInitiated).
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// [Bond Lost](::event::VendorEvent::GapBondLost)
        const BOND_LOST = 0x0020;
    }
}

/// Parameters for the [GAP Limited Discovery](GapCommands::start_limited_discovery_procedure) and
/// [GAP General Discovery](GapCommands::start_general_discovery_procedure) procedures.
pub struct DiscoveryProcedureParameters {
    /// Scanning window for the discovery procedure.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// If true, duplicate devices are filtered out.
    pub filter_duplicates: bool,
}

/// Parameters for the GAP Name Discovery
/// procedure.
pub struct NameDiscoveryProcedureParameters {
    /// Scanning window for the discovery procedure.
    pub scan_window: ScanWindow,

    /// Address of the connected device
    pub peer_address: crate::host::PeerAddrType,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Connection interval parameters.
    pub conn_interval: ConnectionInterval,

    /// Expected connection length
    pub expected_connection_length: ExpectedConnectionLength,
}

/// Parameters for the
/// [GAP Start Auto Connection Establishment](GapCommands::start_auto_connection_establishment_procedure) command.
pub struct AutoConnectionEstablishmentParameters<'a> {
    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Connection interval parameters.
    pub conn_interval: ConnectionInterval,

    /// Expected connection length
    pub expected_connection_length: ExpectedConnectionLength,

    /// Addresses to white-list for automatic connection.
    pub white_list: &'a [crate::host::PeerAddrType],
}

impl<'a> AutoConnectionEstablishmentParameters<'a> {
    fn validate(&self) -> Result<(), Error> {
        const MAX_WHITE_LIST_LENGTH: usize = 33;
        if self.white_list.len() > MAX_WHITE_LIST_LENGTH {
            return Err(Error::WhiteListTooLong);
        }

        Ok(())
    }
}

/// Parameters for the
/// [GAP Start General Connection Establishment](GapCommands::start_general_connection_establishment_procedure) command.
pub struct GeneralConnectionEstablishmentParameters {
    /// passive or active scanning. With passive scanning, no scan request PDUs are sent
    pub scan_type: ScanType,

    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Scanning filter policy.
    ///
    /// # Note
    /// if privacy is enabled, filter policy can only assume values
    /// [Accept All](ScanFilterPolicy::AcceptAll) or
    /// [Addressed To This Device](ScanFilterPolicy::AddressedToThisDevice)
    pub filter_policy: ScanFilterPolicy,

    /// If true, only report unique devices.
    pub filter_duplicates: bool,
}

/// Parameters for the
/// [GAP Start Selective Connection Establishment](GapCommands::start_selective_connection_establishment_procedure) command.
pub struct SelectiveConnectionEstablishmentParameters<'a> {
    /// Type of scanning
    pub scan_type: crate::host::ScanType,

    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Scanning filter policy.
    ///
    /// # Note
    /// if privacy is enabled, filter policy can only assume values
    /// [Accept All](ScanFilterPolicy::AcceptAll) or
    /// [Whitelist Addressed to this Device](ScanFilterPolicy::WhiteListAddressedToThisDevice)
    pub filter_policy: ScanFilterPolicy,

    /// If true, only report unique devices.
    pub filter_duplicates: bool,

    /// Addresses to white-list for automatic connection.
    pub white_list: &'a [crate::host::PeerAddrType],
}

impl<'a> SelectiveConnectionEstablishmentParameters<'a> {
    fn validate(&self) -> Result<(), Error> {
        const MAX_WHITE_LIST_LENGTH: usize = 35;
        if self.white_list.len() > MAX_WHITE_LIST_LENGTH {
            return Err(Error::WhiteListTooLong);
        }

        Ok(())
    }
}

/// The parameters for the GAP Name Discovery
/// and [GAP Create Connection](GapCommands::create_connection) commands are identical.
pub type ConnectionParameters = NameDiscoveryProcedureParameters;

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Roles for a [GAP service](GapCommands::init).
    pub struct Procedure: u8 {
        /// [Limited Discovery](GapCommands::start_limited_discovery_procedure) procedure.
        const LIMITED_DISCOVERY = 0x01;
        /// [General Discovery](GapCommands::start_general_discovery_procedure) procedure.
        const GENERAL_DISCOVERY = 0x02;
        /// Name Discovery procedure.
        const NAME_DISCOVERY = 0x04;
        /// [Auto Connection Establishment](GapCommands::start_auto_connection_establishment_procedure).
        const AUTO_CONNECTION_ESTABLISHMENT = 0x08;
        /// [General Connection Establishment](GapCommands::start_general_connection_establishment_procedure).
        const GENERAL_CONNECTION_ESTABLISHMENT = 0x10;
        /// [Selective Connection Establishment](GapCommands::start_selective_connection_establishment_procedure).
        const SELECTIVE_CONNECTION_ESTABLISHMENT = 0x20;
        /// Direct Connection Establishment.
        const DIRECT_CONNECTION_ESTABLISHMENT = 0x40;
        /// [Observation](GapCommands::start_observation_procedure) procedure.
        const OBSERVATION = 0x80;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Roles for a [GAP service](GapCommands::init).
    pub struct Procedure: u8 {
        /// [Limited Discovery](GapCommands::start_limited_discovery_procedure) procedure.
        const LIMITED_DISCOVERY = 0x01;
        /// [General Discovery](GapCommands::start_general_discovery_procedure) procedure.
        const GENERAL_DISCOVERY = 0x02;
        /// Name Discovery procedure.
        const NAME_DISCOVERY = 0x04;
        /// [Auto Connection Establishment](GapCommands::start_auto_connection_establishment_procedure).
        const AUTO_CONNECTION_ESTABLISHMENT = 0x08;
        /// [General Connection Establishment](GapCommands::start_general_connection_establishment_procedure).
        const GENERAL_CONNECTION_ESTABLISHMENT = 0x10;
        /// [Selective Connection Establishment](GapCommands::start_selective_connection_establishment_procedure).
        const SELECTIVE_CONNECTION_ESTABLISHMENT = 0x20;
        /// Direct Connection Establishment.
        const DIRECT_CONNECTION_ESTABLISHMENT = 0x40;
        /// [Observation](GapCommands::start_observation_procedure) procedure.
        const OBSERVATION = 0x80;
    }
}

/// Parameters for the [`start_connection_update`](GapCommands::start_connection_update)
/// command.
pub struct ConnectionUpdateParameters {
    /// Handle of the connection for which the update procedure has to be started.
    pub conn_handle: crate::ConnectionHandle,

    /// Updated connection interval for the connection.
    pub conn_interval: ConnectionInterval,

    /// Expected length of connection event needed for this connection.
    pub expected_connection_length: ExpectedConnectionLength,
}

/// Parameters for the [`send_pairing_request`](GapCommands::send_pairing_request)
/// command.
pub struct PairingRequest {
    /// Handle of the connection for which the pairing request has to be sent.
    pub conn_handle: crate::ConnectionHandle,

    /// Whether pairing request has to be sent if the device is previously bonded or not. If false,
    /// the pairing request is sent only if the device has not previously bonded.
    pub force_rebond: bool,
}

/// Parameters for the [GAP Set Broadcast Mode](GapCommands::set_broadcast_mode) command.
pub struct BroadcastModeParameters<'a, 'b> {
    /// Advertising type and interval.
    ///
    /// Only the [ScannableUndirected](crate::types::AdvertisingType::ScannableUndirected) and
    /// [NonConnectableUndirected](crate::types::AdvertisingType::NonConnectableUndirected).
    pub advertising_interval: crate::types::AdvertisingInterval,

    /// Type of this device's address.
    ///
    /// A privacy enabled device uses either a
    /// [resolvable private address](AddressType::ResolvablePrivate) or a
    /// [non-resolvable private](AddressType::NonResolvablePrivate) address.
    pub own_address_type: AddressType,

    /// Advertising data used by the device when advertising.
    ///
    /// Must be 31 bytes or fewer.
    pub advertising_data: &'a [u8],

    /// Addresses to add to the white list.
    ///
    /// Each address takes up 7 bytes (1 byte for the type, 6 for the address). The full length of
    /// this packet must not exceed 255 bytes. The white list must be less than a maximum of between
    /// 31 and 35 entries, depending on the length of
    /// [`advertising_data`](BroadcastModeParameters::advertising_data). Shorter advertising data
    /// allows more white list entries.
    pub white_list: &'b [crate::host::PeerAddrType],
}

impl<'a, 'b> BroadcastModeParameters<'a, 'b> {
    const MAX_LENGTH: usize = 255;

    fn validate(&self) -> Result<(), Error> {
        const MAX_ADVERTISING_DATA_LENGTH: usize = 31;

        match self.advertising_interval.advertising_type() {
            crate::types::AdvertisingType::ScannableUndirected
            | crate::types::AdvertisingType::NonConnectableUndirected => (),
            other => return Err(Error::BadAdvertisingType(other)),
        }

        if self.advertising_data.len() > MAX_ADVERTISING_DATA_LENGTH {
            return Err(Error::BadAdvertisingDataLength(self.advertising_data.len()));
        }

        if self.len() > Self::MAX_LENGTH {
            return Err(Error::WhiteListTooLong);
        }

        Ok(())
    }

    fn len(&self) -> usize {
        5 + // advertising_interval
            1 + // own_address_type
            1 + self.advertising_data.len() + // advertising_data
            1 + 7 * self.white_list.len() // white_list
    }
}

/// Parameters for the [GAP Start Observation Procedure](GapCommands::start_observation_procedure)
/// command.
pub struct ObservationProcedureParameters {
    /// Scanning window.
    pub scan_window: crate::types::ScanWindow,

    /// Active or passive scanning
    pub scan_type: crate::host::ScanType,

    /// Address type of this device.
    pub own_address_type: AddressType,

    /// If true, do not report duplicate events in the
    /// [advertising report](crate::event::Event::LeAdvertisingReport).
    pub filter_duplicates: bool,

    /// Scanning filter policy
    pub filter_policy: ScanFilterPolicy,
}

/// Parameters for [GAP Numeric Comparison Confirm Yes or No](crate::vendor::command::gap::GapCommands::numeric_comparison_value_confirm_yes_no)
pub struct NumericComparisonValueConfirmYesNoParameters {
    /// Connection handle for which the command applies.
    pub conn_handle: ConnectionHandle,

    /// Indicates if the numeric values shown on both local and peer device are different or equal.
    pub confirm_yes_no: bool,
}

/// Parameter for [GAP Passkey Input](GapCommands::passkey_input)
pub enum InputType {
    EntryStarted = 0x00,
    DigitEntered = 0x01,
    DigitErased = 0x02,
    Cleared = 0x03,
    EntryCompleted = 0x04,
}

#[derive(Clone, Copy)]
pub enum OobDataType {
    /// TK (LP v.4.1)
    TK,
    /// Random (SC)
    Random,
    /// Confirm (SC)
    Confirm,
}

#[derive(Clone, Copy)]
pub enum OobDeviceType {
    Local = 0x00,
    Remote = 0x01,
}

/// Parameters for [GAP Set OOB Data](GapCommands::set_oob_data)
pub struct SetOobDataParameters {
    /// OOB Device type
    pub device_type: OobDeviceType,
    /// Identity address
    pub address: BdAddrType,
    /// OOB Data type
    pub oob_data_type: OobDataType,
    /// Pairing Data received through OOB from remote device
    pub oob_data: [u8; 16],
}

/// Parameter for [GAP Add Devices to List](GapCommands::add_devices_to_list)
pub enum AddDeviceToListMode {
    /// Append to the resolving list only
    AppendResoling = 0x00,
    /// clear and set the resolving list only
    ClearAndSetResolving = 0x01,
    /// append to the whitelist only
    AppendWhitelist = 0x02,
    /// clear and set the whitelist only
    ClearAndSetWhitelist = 0x03,
    /// apppend to both resolving and white lists
    AppendBoth = 0x04,
    /// clear and set both resolving and white lists
    ClearAndSetBoth = 0x05,
}

/// Parameters for [GAP Additional Beacon Start](GapCommands::additional_beacon_start)
pub struct AdditonalBeaconStartParameters {
    /// Advertising interval
    pub advertising_interval: (Duration, Duration),
    /// advertising channel map
    pub advertising_channel_map: Channels,
    /// Own address type
    pub own_address_type: BdAddrType,
    /// Power amplifier output level. Range: 0x00 .. 0x23
    pub pa_level: u8,
}

impl AdditonalBeaconStartParameters {
    fn validate(&self) -> Result<(), Error> {
        const AMPLIFIER_MAX: u8 = 0x23;

        if self.pa_level > AMPLIFIER_MAX {
            return Err(Error::BadPowerAmplifierLevel(self.pa_level));
        }

        Ok(())
    }
}

/// Params for the [adv_set_config](GapCommands::adv_set_config) command
pub struct AdvSetConfig {
    /// Bitmap of extended advertising modes
    pub adv_mode: AdvertisingMode,
    /// Used to identify an advertising set
    pub adv_handle: AdvertisingHandle,
    /// Type of advertising event
    pub adv_event_properties: AdvertisingEvent,
    /// Advertising interval
    pub adv_interval: ExtendedAdvertisingInterval,
    /// Advertising channel map
    pub primary_adv_channel_map: Channels,
    /// Own address type.
    ///
    /// If privacy is disabled, the address can be public or static random, otherwise,
    /// it can be a resolvable private address or a non-resolvabble private address.
    pub own_addr_type: OwnAddressType,
    /// Public device address, random device addressm public identity address, or random
    /// (static) identity address of the device to be connected.
    pub peer_addr: BdAddrType,
    /// Advertising filter policy
    pub adv_filter_policy: AdvertisingFilterPolicy,
    /// Advertising TX power. Units; dBm.
    ///
    /// Values;
    /// - -127 .. 20
    pub adv_tx_power: u8,
    /// Secondary advertising maximum skip.
    ///
    /// Values:
    /// - 0x00: `AUX_QDV_IND` shall be sent prior to the next advertising event
    /// - 0x01 .. 0xFF: Maximum advertising events to the Controller can skip
    ///   before sending the `AUX_QDV_IND` packets on the secondary physical channel.
    pub secondary_adv_max_skip: u8,
    /// Secondary advertising PHY
    pub secondary_adv_phy: AdvertisingPhy,
    /// Value of advertising SID subfield in the ADI field of the PDU.
    ///
    /// Values:
    /// - 0x00 .. 0x0F
    pub adv_sid: u8,
    /// Scan request notifications
    pub scan_req_notification_enable: bool,
}

/// Params for the [adv_set_enable](GapCommands::adv_set_enable) command
pub struct AdvSetEnable<'a> {
    /// Enable/Disable advertising
    pub enable: bool,
    /// Number of advertising sets.
    ///
    /// Values
    /// - 0x00: disable all advertising sets
    /// - 0x01 .. 0x3F: Number of advertising sets to enable or disable
    pub num_sets: u8,
    /// Advertising sets
    pub adv_set: &'a [AdvSet],
}

/// Params for the [adv_set_advertising_data](GapCommands::adv_set_advertising_data) command
pub struct AdvSetAdvertisingData<'a> {
    /// Used to identify an advertising set
    pub adv_handle: AdvertisingHandle,
    /// Advertising operation
    pub operation: AdvertisingOperation,
    /// Fragment preference. If set to `true`, the Controller may fragment all data, else
    /// the Controller should not fragment or should minimize fragmentation of data
    pub fragment: bool,
    /// Data formatted as defined in Bluetooth spec. v.5.4 [Vol 3, Part C, 11].
    pub data: &'a [u8],
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_periodic_parameters](GapCommands::adv_set_periodic_parameters).
pub struct AdvSetPeriodicParameters {
    pub advertising_handle: AdvertisingHandle,
    pub periodic_adv_interval_min: u16,
    pub periodic_adv_interval_max: u16,
    pub periodic_adv_properties: u16,
    pub num_subevents: u8,
    pub subevent_interval: u8,
    pub response_slot_delay: u8,
    pub response_slot_spacing: u8,
    pub num_response_slots: u8,
}

#[cfg(after_fw_0_17_1)]
impl AdvSetPeriodicParameters {
    pub(crate) const LENGTH: usize = 12;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert!(bytes.len() >= Self::LENGTH);
        bytes[0] = self.advertising_handle.0;
        LittleEndian::write_u16(&mut bytes[1..3], self.periodic_adv_interval_min);
        LittleEndian::write_u16(&mut bytes[3..5], self.periodic_adv_interval_max);
        LittleEndian::write_u16(&mut bytes[5..7], self.periodic_adv_properties);
        bytes[7] = self.num_subevents;
        bytes[8] = self.subevent_interval;
        bytes[9] = self.response_slot_delay;
        bytes[10] = self.response_slot_spacing;
        bytes[11] = self.num_response_slots;
    }
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_periodic_data](GapCommands::adv_set_periodic_data).
pub struct AdvSetPeriodicData<'a> {
    pub advertising_handle: AdvertisingHandle,
    pub operation: AdvertisingOperation,
    pub data: &'a [u8],
}

#[cfg(after_fw_0_17_1)]
impl<'a> AdvSetPeriodicData<'a> {
    pub(crate) const MAX_LENGTH: usize = 255;

    fn copy_into_slice(&self, bytes: &mut [u8]) -> usize {
        assert!(bytes.len() >= Self::MAX_LENGTH);
        bytes[0] = self.advertising_handle.0;
        bytes[1] = self.operation as u8;
        let len = self.data.len();
        bytes[2] = len as u8;
        bytes[3..3 + len].copy_from_slice(self.data);
        3 + len
    }
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_configuration_v2](GapCommands::adv_set_configuration_v2).
///
/// Like [AdvSetConfig] but uses 4-byte primary advertising intervals and adds PHY fields.
pub struct AdvSetConfigV2 {
    pub adv_mode: AdvertisingMode,
    pub adv_handle: AdvertisingHandle,
    pub adv_event_properties: AdvertisingEvent,
    /// Minimum primary advertising interval (N * 0.625 ms).
    pub primary_adv_interval_min: u32,
    /// Maximum primary advertising interval (N * 0.625 ms).
    pub primary_adv_interval_max: u32,
    pub primary_adv_channel_map: Channels,
    pub own_addr_type: OwnAddressType,
    pub peer_addr: BdAddrType,
    pub adv_filter_policy: AdvertisingFilterPolicy,
    pub adv_tx_power: u8,
    pub primary_adv_phy: AdvertisingPhy,
    pub secondary_adv_max_skip: u8,
    pub secondary_adv_phy: AdvertisingPhy,
    pub adv_sid: u8,
    pub scan_req_notification_enable: bool,
    pub primary_adv_phy_options: u8,
}

#[cfg(after_fw_0_17_1)]
impl AdvSetConfigV2 {
    pub(crate) const LENGTH: usize = 29;

    fn copy_into_slice(&self, bytes: &mut [u8]) {
        assert!(bytes.len() >= Self::LENGTH);
        bytes[0] = self.adv_mode.bits();
        bytes[1] = self.adv_handle.0;
        LittleEndian::write_u16(&mut bytes[2..4], self.adv_event_properties.bits());
        LittleEndian::write_u32(&mut bytes[4..8], self.primary_adv_interval_min);
        LittleEndian::write_u32(&mut bytes[8..12], self.primary_adv_interval_max);
        bytes[12] = self.primary_adv_channel_map.bits();
        bytes[13] = self.own_addr_type as u8;
        self.peer_addr.copy_into_slice(&mut bytes[14..]);
        bytes[21] = self.adv_filter_policy as u8;
        bytes[22] = self.adv_tx_power;
        bytes[23] = self.primary_adv_phy as u8;
        bytes[24] = self.secondary_adv_max_skip;
        bytes[25] = self.secondary_adv_phy as u8;
        bytes[26] = self.adv_sid;
        bytes[27] = self.scan_req_notification_enable as u8;
        bytes[28] = self.primary_adv_phy_options;
    }
}

/// One record in the extended-scan PHY parameter list.
pub struct ExtScanPhyParams {
    pub scan_type: u8,
    pub scan_interval: u16,
    pub scan_window: u16,
}

impl crate::vendor::command::HciEncodeField<5> for ExtScanPhyParams {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[self.scan_type])?;
        writer.write_all(&self.scan_interval.to_le_bytes())?;
        writer.write_all(&self.scan_window.to_le_bytes())
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[self.scan_type]).await?;
        writer.write_all(&self.scan_interval.to_le_bytes()).await?;
        writer.write_all(&self.scan_window.to_le_bytes()).await
    }
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [ext_start_scan](GapCommands::ext_start_scan).
pub struct ExtStartScanParams {
    pub scan_mode: u8,
    pub procedure: u8,
    pub own_address_type: u8,
    pub filter_duplicates: u8,
    pub duration: u16,
    pub period: u16,
    pub scanning_filter_policy: u8,
    pub scanning_phys: u8,
    /// Per-PHY parameters (one entry per set bit in scanning_phys, max 2).
    pub phy_params: [ExtScanPhyParams; 2],
    pub num_phys: usize,
}

#[cfg(after_fw_0_17_1)]
/// Per-PHY connection parameters for [ExtCreateConnectionParams].
pub struct ExtConnPhyParams {
    pub scan_interval: u16,
    pub scan_window: u16,
    pub conn_interval_min: u16,
    pub conn_interval_max: u16,
    pub conn_latency: u16,
    pub supervision_timeout: u16,
    pub min_ce_length: u16,
    pub max_ce_length: u16,
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [ext_create_connection](GapCommands::ext_create_connection).
pub struct ExtCreateConnectionParams {
    pub initiating_mode: u8,
    pub procedure: u8,
    pub own_address_type: u8,
    pub peer_address_type: u8,
    pub peer_address: BdAddr,
    pub advertising_handle: u8,
    pub subevent: u8,
    pub initiator_filter_policy: u8,
    pub initiating_phys: u8,
    /// Per-PHY parameters (one entry per set bit in initiating_phys, max 3).
    pub phy_params: [ExtConnPhyParams; 3],
    pub num_phys: usize,
}

#[cfg(after_fw_0_17_1)]
impl ExtCreateConnectionParams {
    pub(crate) const MAX_LENGTH: usize = 14 + 3 * 16;

    fn copy_into_slice(&self, bytes: &mut [u8]) -> usize {
        assert!(bytes.len() >= Self::MAX_LENGTH);
        bytes[0] = self.initiating_mode;
        bytes[1] = self.procedure;
        bytes[2] = self.own_address_type;
        bytes[3] = self.peer_address_type;
        bytes[4..10].copy_from_slice(&self.peer_address.0);
        bytes[10] = self.advertising_handle;
        bytes[11] = self.subevent;
        bytes[12] = self.initiator_filter_policy;
        bytes[13] = self.initiating_phys;
        let mut offset = 14;
        for i in 0..self.num_phys.min(3) {
            let p = &self.phy_params[i];
            LittleEndian::write_u16(&mut bytes[offset..], p.scan_interval);
            LittleEndian::write_u16(&mut bytes[offset + 2..], p.scan_window);
            LittleEndian::write_u16(&mut bytes[offset + 4..], p.conn_interval_min);
            LittleEndian::write_u16(&mut bytes[offset + 6..], p.conn_interval_max);
            LittleEndian::write_u16(&mut bytes[offset + 8..], p.conn_latency);
            LittleEndian::write_u16(&mut bytes[offset + 10..], p.supervision_timeout);
            LittleEndian::write_u16(&mut bytes[offset + 12..], p.min_ce_length);
            LittleEndian::write_u16(&mut bytes[offset + 14..], p.max_ce_length);
            offset += 16;
        }
        offset
    }
}
