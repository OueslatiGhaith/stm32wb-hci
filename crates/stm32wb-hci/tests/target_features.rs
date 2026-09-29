//! The target-selecting Cargo features must match the bundled catalog.

use std::collections::BTreeSet;

use stm32wb_catalog::{Platform, Profile, bundled};

fn declared_features() -> BTreeSet<String> {
    let manifest = include_str!("../Cargo.toml")
        .parse::<toml::Table>()
        .unwrap();
    manifest["features"]
        .as_table()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn release_features_match_the_catalog() {
    let declared = declared_features()
        .into_iter()
        .filter(|feature| feature.starts_with("fw_"))
        .collect::<BTreeSet<_>>();
    let catalog = bundled(Platform::Stm32wb)
        .unwrap()
        .catalog
        .versions()
        .map(|release| Platform::Stm32wb.release_feature(release))
        .collect::<BTreeSet<_>>();
    assert_eq!(declared, catalog);
}

#[test]
fn profile_features_match_the_catalog() {
    let declared = declared_features()
        .into_iter()
        .filter(|feature| feature.starts_with("stack-"))
        .collect::<BTreeSet<_>>();
    let profiles = bundled(Platform::Stm32wb)
        .unwrap()
        .catalog
        .platform
        .profiles()
        .iter()
        .map(|profile| profile.feature_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(declared, profiles);
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
