//! ST vendor (ACI) commands declared against the STM32WB catalog.
//!
//! Each command's opcode, completion kind, and availability come from the
//! catalog for the release and stack profile selected by the `fw_*` and
//! `stack-*` features; a command the selected binary does not implement is
//! not compiled.

pub mod att;
pub mod flags;
pub mod gap;
pub mod gatt;
pub mod general;
pub mod hal;
pub mod l2cap;
pub mod values;

stm32wb_hci_macros::vendor_events! {
    /// Every ST vendor event the selected target emits. Decode one from a
    /// bt-hci [`Vendor`](bt_hci::event::Vendor) event with
    /// [`AciEvent::from_vendor`].
    pub enum AciEvent<'a> {
        aci_hal_end_of_radio_activity_event => HalEndOfRadioActivity(hal::HalEndOfRadioActivityEvent),
        aci_hal_scan_req_report_event => HalScanReqReport(hal::HalScanReqReportEvent),
        aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
        aci_gap_limited_discoverable_event => GapLimitedDiscoverable(gap::GapLimitedDiscoverableEvent),
        aci_gap_pairing_complete_event => GapPairingComplete(gap::GapPairingCompleteEvent),
        aci_gap_pass_key_req_event => GapPassKeyReq(gap::GapPassKeyReqEvent),
        aci_gap_authorization_req_event => GapAuthorizationReq(gap::GapAuthorizationReqEvent),
        aci_gap_peripheral_security_initiated_event => GapPeripheralSecurityInitiated(gap::GapPeripheralSecurityInitiatedEvent),
        aci_gap_bond_lost_event => GapBondLost(gap::GapBondLostEvent),
        aci_gap_proc_complete_event => GapProcComplete(gap::GapProcCompleteEvent<'a>),
        aci_gap_addr_not_resolved_event => GapAddrNotResolved(gap::GapAddrNotResolvedEvent),
        aci_gap_numeric_comparison_value_event => GapNumericComparisonValue(gap::GapNumericComparisonValueEvent),
        aci_gap_keypress_notification_event => GapKeypressNotification(gap::GapKeypressNotificationEvent),
        aci_gap_pairing_request_event => GapPairingRequest(gap::GapPairingRequestEvent),
        aci_l2cap_connection_update_resp_event => L2capConnectionUpdateResp(l2cap::L2capConnectionUpdateRespEvent),
        aci_l2cap_proc_timeout_event => L2capProcTimeout(l2cap::L2capProcTimeoutEvent<'a>),
        aci_l2cap_connection_update_req_event => L2capConnectionUpdateReq(l2cap::L2capConnectionUpdateReqEvent),
        aci_l2cap_command_reject_event => L2capCommandReject(l2cap::L2capCommandRejectEvent<'a>),
        aci_l2cap_coc_connect_event => L2capCocConnect(l2cap::L2capCocConnectEvent),
        aci_l2cap_coc_connect_confirm_event => L2capCocConnectConfirm(l2cap::L2capCocConnectConfirmEvent<'a>),
        aci_l2cap_coc_reconf_event => L2capCocReconf(l2cap::L2capCocReconfEvent<'a>),
        aci_l2cap_coc_reconf_confirm_event => L2capCocReconfConfirm(l2cap::L2capCocReconfConfirmEvent),
        aci_l2cap_coc_disconnect_event => L2capCocDisconnect(l2cap::L2capCocDisconnectEvent),
        aci_l2cap_coc_flow_control_event => L2capCocFlowControl(l2cap::L2capCocFlowControlEvent),
        aci_l2cap_coc_rx_data_event => L2capCocRxData(l2cap::L2capCocRxDataEvent<'a>),
        aci_l2cap_coc_tx_pool_available_event => L2capCocTxPoolAvailable(l2cap::L2capCocTxPoolAvailableEvent),
        aci_gatt_attribute_modified_event => GattAttributeModified(gatt::GattAttributeModifiedEvent<'a>),
        aci_gatt_proc_timeout_event => GattProcTimeout(gatt::GattProcTimeoutEvent),
        aci_gatt_indication_event => GattIndication(gatt::GattIndicationEvent<'a>),
        aci_gatt_notification_event => GattNotification(gatt::GattNotificationEvent<'a>),
        aci_gatt_proc_complete_event => GattProcComplete(gatt::GattProcCompleteEvent),
        aci_gatt_error_resp_event => GattErrorResp(gatt::GattErrorRespEvent),
        aci_gatt_disc_read_char_by_uuid_resp_event => GattDiscReadCharByUuidResp(gatt::GattDiscReadCharByUuidRespEvent<'a>),
        aci_gatt_write_permit_req_event => GattWritePermitReq(gatt::GattWritePermitReqEvent<'a>),
        aci_gatt_read_permit_req_event => GattReadPermitReq(gatt::GattReadPermitReqEvent),
        aci_gatt_read_multi_permit_req_event => GattReadMultiPermitReq(gatt::GattReadMultiPermitReqEvent<'a>),
        aci_gatt_tx_pool_available_event => GattTxPoolAvailable(gatt::GattTxPoolAvailableEvent),
        aci_gatt_server_confirmation_event => GattServerConfirmation(gatt::GattServerConfirmationEvent),
        aci_gatt_prepare_write_permit_req_event => GattPrepareWritePermitReq(gatt::GattPrepareWritePermitReqEvent<'a>),
        aci_gatt_eatt_bearer_event => GattEattBearer(gatt::GattEattBearerEvent),
        aci_gatt_mult_notification_event => GattMultNotification(gatt::GattMultNotificationEvent<'a>),
        aci_gatt_notification_complete_event => GattNotificationComplete(gatt::GattNotificationCompleteEvent),
        aci_gatt_read_ext_event => GattReadExt(gatt::GattReadExtEvent<'a>),
        aci_gatt_indication_ext_event => GattIndicationExt(gatt::GattIndicationExtEvent<'a>),
        aci_gatt_notification_ext_event => GattNotificationExt(gatt::GattNotificationExtEvent<'a>),
        aci_att_exchange_mtu_resp_event => AttExchangeMtuResp(att::AttExchangeMtuRespEvent),
        aci_att_find_info_resp_event => AttFindInfoResp(att::AttFindInfoRespEvent<'a>),
        aci_att_find_by_type_value_resp_event => AttFindByTypeValueResp(att::AttFindByTypeValueRespEvent<'a>),
        aci_att_read_by_type_resp_event => AttReadByTypeResp(att::AttReadByTypeRespEvent<'a>),
        aci_att_read_resp_event => AttReadResp(att::AttReadRespEvent<'a>),
        aci_att_read_blob_resp_event => AttReadBlobResp(att::AttReadBlobRespEvent<'a>),
        aci_att_read_multiple_resp_event => AttReadMultipleResp(att::AttReadMultipleRespEvent<'a>),
        aci_att_read_by_group_type_resp_event => AttReadByGroupTypeResp(att::AttReadByGroupTypeRespEvent<'a>),
        aci_att_prepare_write_resp_event => AttPrepareWriteResp(att::AttPrepareWriteRespEvent<'a>),
        aci_att_exec_write_resp_event => AttExecWriteResp(att::AttExecWriteRespEvent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::VendorEvent;

    #[test]
    fn events_decode_into_the_variant_their_code_selects() {
        let code = hal::HalEndOfRadioActivityEvent::CODE.to_le_bytes();
        let params = [code[0], code[1], 1, 2, 0x10, 0x20, 0x30, 0x40, 3, 4];
        let Some(Ok(AciEvent::HalEndOfRadioActivity(activity))) =
            AciEvent::from_vendor_params(&params)
        else {
            panic!("the code selects the end of radio activity");
        };
        assert_eq!(activity.next_state_sys_time, 0x4030_2010);
        assert!(AciEvent::from_vendor_params(&[0xFF, 0xFF]).is_none());
        assert!(AciEvent::from_vendor_params(&[0x04]).is_none());
        assert!(AciEvent::from_vendor_params(&params[..9]).unwrap().is_err());
    }

    #[cfg(any(feature = "stack-full-extended", feature = "stack-full"))]
    #[test]
    fn borrowed_events_decode_in_place() {
        let params = [0x0F, 0x0C, 0x01, 0x00, 0x12, 0x00, 2, 0xAA, 0xBB];
        let Some(Ok(AciEvent::GattNotification(notification))) =
            AciEvent::from_vendor_params(&params)
        else {
            panic!("0x0C0F is the GATT notification");
        };
        assert_eq!(notification.attribute_value, [0xAA, 0xBB]);
    }

    /// Where no event borrows, the hidden variant using the lifetime needs no
    /// match arm.
    #[cfg(all(feature = "stack-hci-adv-scan", feature = "fw_1_15_0"))]
    #[test]
    fn unborrowed_targets_match_exhaustively() {
        let params = [0x04, 0x00, 1, 2, 0x10, 0x20, 0x30, 0x40, 3, 4];
        let AciEvent::HalEndOfRadioActivity(activity) =
            AciEvent::from_vendor_params(&params).unwrap().unwrap();
        assert_eq!(activity.last_state, 1);
    }
}
