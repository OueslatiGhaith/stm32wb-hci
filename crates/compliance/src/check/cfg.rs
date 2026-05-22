//! Firmware cfg evaluation for Rust-side compliance scanning.
//!
//! The HCI crate uses generated cfgs such as `since_fw_v1_15_0`,
//! `before_fw_v1_15_0`, and `fw_v1_15_0` to hide items that do not exist for
//! the selected controller firmware. The compliance checker evaluates those
//! cfgs against the firmware tag being checked so gated Rust items do not show
//! up as unknown markers, methods, opcodes, or events.

use proc_macro2::{TokenStream, TokenTree};
use syn::Attribute;

/// Firmware version used to evaluate `fw_*`, `since_fw_*`, and `before_fw_*` cfgs.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct FirmwareCfg {
    major: u16,
    minor: u16,
    patch: u16,
}

impl FirmwareCfg {
    /// Parses a firmware label such as `v1.15.0`.
    pub(super) fn parse(label: &str) -> Option<Self> {
        Self::parse_version(label.trim().strip_prefix('v')?)
    }

    /// Returns whether all firmware cfg attributes allow an item for this firmware.
    pub(super) fn allows_attrs(&self, attrs: &[Attribute]) -> bool {
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .all(|attr| {
                self.allows_cfg_tokens(
                    attr.meta
                        .require_list()
                        .ok()
                        .map(|meta| meta.tokens.clone()),
                )
            })
    }

    /// Returns whether a raw `cfg(...)` token stream allows an item for this firmware.
    pub(super) fn allows_cfg_stream(&self, tokens: TokenStream) -> bool {
        self.allows_cfg_tokens(Some(tokens))
    }

    fn allows_cfg_tokens(&self, tokens: Option<TokenStream>) -> bool {
        let Some(tokens) = tokens else {
            return true;
        };
        let tokens = tokens.into_iter().collect::<Vec<_>>();
        let Some(TokenTree::Ident(ident)) = tokens.first() else {
            return true;
        };
        let cfg = ident.to_string();

        if let Some(version) = cfg.strip_prefix("since_fw_v").and_then(Self::parse_version) {
            *self >= version
        } else if let Some(version) = cfg
            .strip_prefix("before_fw_v")
            .and_then(Self::parse_version)
        {
            *self < version
        } else if let Some(version) = cfg.strip_prefix("fw_v").and_then(Self::parse_version) {
            *self == version
        } else {
            true
        }
    }

    fn parse_version(version: &str) -> Option<Self> {
        let parts = version.split(['_', '.']).collect::<Vec<_>>();
        let [major, minor, patch] = parts.as_slice() else {
            return None;
        };

        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            patch: patch.parse().ok()?,
        })
    }
}
