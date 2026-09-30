//! The target-selecting Cargo features must match the bundled catalogs.

use std::collections::BTreeSet;

use stm32wb_catalog::{Platform, Profile, bundled};

fn features() -> toml::Table {
    include_str!("../Cargo.toml")
        .parse::<toml::Table>()
        .unwrap()["features"]
        .as_table()
        .unwrap()
        .clone()
}

/// The features `feature` enables.
fn enabled(features: &toml::Table, feature: &str) -> Vec<String> {
    features[feature]
        .as_array()
        .unwrap()
        .iter()
        .map(|enabled| enabled.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn release_features_match_the_catalogs() {
    let features = features();
    for platform in Platform::ALL {
        let declared = features
            .keys()
            .filter(|feature| feature.starts_with(platform.release_feature_prefix()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let catalog = bundled(platform)
            .unwrap()
            .catalog
            .versions()
            .map(|release| platform.release_feature(release))
            .collect::<BTreeSet<_>>();
        assert_eq!(declared, catalog);
        for feature in &declared {
            assert_eq!(
                enabled(&features, feature),
                [platform.feature()],
                "{feature}"
            );
        }
    }
}

#[test]
fn profile_features_match_the_catalogs() {
    let features = features();
    let declared = features
        .keys()
        .filter(|feature| feature.starts_with("stack-"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let profiles = Profile::ALL
        .iter()
        .map(|profile| profile.feature_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(declared, profiles);
    for platform in Platform::ALL {
        assert_eq!(bundled(platform).unwrap().catalog.platform, platform);
        for profile in platform.profiles() {
            assert_eq!(
                enabled(&features, &profile.feature_name()),
                [platform.feature()],
                "{profile}"
            );
        }
    }
}

#[test]
fn platform_features_enable_nothing() {
    let features = features();
    for platform in Platform::ALL {
        assert!(enabled(&features, platform.feature()).is_empty());
    }
}

#[test]
fn the_default_target_is_the_newest_full_extended_binary() {
    let manifest = include_str!("../Cargo.toml")
        .parse::<toml::Table>()
        .unwrap();
    let newest = bundled(Platform::Stm32wb)
        .unwrap()
        .catalog
        .versions()
        .last()
        .unwrap();
    let default = manifest["features"]["default"].as_array().unwrap();
    assert!(
        default.contains(&Platform::Stm32wb.release_feature(newest).into()),
        "{default:?}"
    );
    assert!(
        default.contains(&Profile::FullExtended.feature_name().into()),
        "{default:?}"
    );
}
