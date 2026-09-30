//! `att_bearer_range!`: the last enhanced ATT bearer each release of each
//! platform documents.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;
use stm32wb_catalog::{Bundled, ReleaseRange, Version};

/// `LAST_ENHANCED` for each group of the catalog's releases documenting the
/// same range, which every bearer member of a release must share, compiled
/// only for those releases.
pub fn expand(bundled: &Bundled) -> Result<TokenStream, String> {
    let catalog = &bundled.catalog;
    let definitions = catalog
        .commands
        .iter()
        .flat_map(|command| {
            command
                .definitions
                .iter()
                .map(move |definition| (command.name(), definition.releases, &definition.bearers))
        })
        .chain(catalog.events.iter().flat_map(|event| {
            event
                .definitions
                .iter()
                .map(move |definition| (event.name(), definition.releases, &definition.bearers))
        }));

    // Each release's last enhanced bearer, and the entry it was first read from.
    let mut releases: BTreeMap<Version, (u16, &str)> = BTreeMap::new();
    for (name, range, bearers) in definitions {
        for bearer in bearers {
            for release in catalog
                .versions()
                .filter(|release| range.contains(*release))
            {
                let (last, first_name) = *releases.entry(release).or_insert((bearer.last, name));
                if last != bearer.last {
                    return Err(format!(
                        "{release} documents enhanced ATT bearers up to 0x{last:04X} for \
                         {first_name} but 0x{:04X} for {name}",
                        bearer.last
                    ));
                }
            }
        }
    }
    let mut groups: BTreeMap<u16, Vec<Version>> = BTreeMap::new();
    for release in catalog.versions() {
        let (last, _) = releases
            .get(&release)
            .ok_or_else(|| format!("{release} documents no enhanced ATT bearer"))?;
        groups.entry(*last).or_default().push(release);
    }

    let all = catalog.versions().collect::<Vec<_>>();
    let several = groups.len() > 1;
    Ok(groups
        .into_iter()
        .map(|(last, releases)| {
            let cfg = if several {
                let features = releases
                    .iter()
                    .map(|release| catalog.platform.release_feature(*release));
                quote!(#[cfg(any(#(feature = #features),*))])
            } else {
                let platform = catalog.platform.feature();
                quote!(#[cfg(feature = #platform)])
            };
            let doc = format!(
                "The last enhanced ATT bearer the selected release documents, 0x{last:04X}, \
                 as in releases {}.",
                describe(&all, &releases)
            );
            quote! {
                #cfg
                #[doc = #doc]
                pub const LAST_ENHANCED: u16 = #last;
            }
        })
        .collect())
}

/// `releases` as runs of consecutive catalog releases, for documentation.
fn describe(all: &[Version], releases: &[Version]) -> String {
    let mut runs: Vec<ReleaseRange> = Vec::new();
    for (index, release) in all.iter().enumerate() {
        if !releases.contains(release) {
            continue;
        }
        match runs.last_mut() {
            Some(run) if index > 0 && run.last == all[index - 1] => run.last = *release,
            _ => runs.push(ReleaseRange::single(*release)),
        }
    }
    runs.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use stm32wb_catalog::{Platform, bundled};

    use super::*;

    #[test]
    fn releases_share_one_range_each() {
        let tokens = expand(bundled(Platform::Stm32wb).unwrap())
            .unwrap()
            .to_string();
        assert_eq!(
            tokens.matches("pub const LAST_ENHANCED").count(),
            2,
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "# [cfg (any (feature = \"fw_1_15_0\" , feature = \"fw_1_16_0\"))] \
                 # [doc = \"The last enhanced ATT bearer the selected release documents, 0xEA1F"
            ),
            "{tokens}"
        );
        assert!(tokens.contains("= 59967u16"), "{tokens}");
    }

    #[test]
    fn each_platform_selects_its_own_releases() {
        let tokens = expand(bundled(Platform::Stm32wba).unwrap())
            .unwrap()
            .to_string();
        // STM32CubeWBA 1.0.0 documents 32 enhanced bearers, and later
        // releases 64.
        assert!(
            tokens.starts_with("# [cfg (any (feature = \"wba_1_0_0\"))]"),
            "{tokens}"
        );
        assert_eq!(
            tokens.matches("pub const LAST_ENHANCED").count(),
            2,
            "{tokens}"
        );
        assert!(tokens.contains("= 59935u16"), "{tokens}");
        assert!(tokens.contains("= 59967u16"), "{tokens}");
        assert!(!tokens.contains("fw_"), "{tokens}");
    }
}
