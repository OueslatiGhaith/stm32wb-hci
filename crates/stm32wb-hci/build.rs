use std::cmp::Ordering;
use std::env;
use std::fs;
use std::path::Path;

const FIRMWARE_FEATURE_PREFIX: &str = "fw_v";

fn main() {
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by Cargo");
    let manifest_path = Path::new(&manifest_dir).join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", manifest_path.display());
    });

    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let firmware_features = firmware_features_from_manifest(&manifest);
    if firmware_features.is_empty() {
        panic!("Cargo.toml must define at least one `{FIRMWARE_FEATURE_PREFIX}*` feature");
    }

    for feature in &firmware_features {
        println!("cargo:rerun-if-env-changed={}", feature.cargo_env_name());
        println!("cargo:rustc-check-cfg=cfg({})", feature.cfg_name());
        println!("cargo:rustc-check-cfg=cfg(since_{})", feature.cfg_name());
        println!("cargo:rustc-check-cfg=cfg(before_{})", feature.cfg_name());
    }

    let selected = firmware_features
        .iter()
        .filter(|feature| env::var_os(feature.cargo_env_name()).is_some())
        .collect::<Vec<_>>();

    match selected.as_slice() {
        [] => panic!(
            "select exactly one target firmware feature: {}",
            feature_list(&firmware_features)
        ),
        [selected] => emit_firmware_cfgs(selected, &firmware_features),
        _ => panic!(
            "select only one target firmware feature, found: {}",
            selected
                .iter()
                .map(|feature| feature.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn emit_firmware_cfgs(selected: &FirmwareFeature, firmware_features: &[FirmwareFeature]) {
    println!("cargo:rustc-cfg={}", selected.cfg_name());

    for feature in firmware_features {
        match selected.version.cmp(&feature.version) {
            Ordering::Less => println!("cargo:rustc-cfg=before_{}", feature.cfg_name()),
            Ordering::Equal | Ordering::Greater => {
                println!("cargo:rustc-cfg=since_{}", feature.cfg_name());
            }
        }
    }
}

fn firmware_features_from_manifest(manifest: &str) -> Vec<FirmwareFeature> {
    let mut features = Vec::new();
    let mut in_features = false;

    for line in manifest.lines() {
        let line = line.split_once('#').map_or(line, |(line, _)| line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            in_features = line == "[features]";
            continue;
        }

        if !in_features {
            continue;
        }

        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_matches('"').to_owned();
        let Some(version) = FirmwareVersion::parse_feature_name(&name) else {
            continue;
        };

        if features
            .iter()
            .any(|feature: &FirmwareFeature| feature.version == version)
        {
            panic!(
                "duplicate target firmware feature for {}",
                version.cfg_suffix()
            );
        }

        features.push(FirmwareFeature { name, version });
    }

    features.sort_by_key(|feature| feature.version);
    features
}

fn feature_list(features: &[FirmwareFeature]) -> String {
    features
        .iter()
        .map(|feature| feature.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug)]
struct FirmwareFeature {
    name: String,
    version: FirmwareVersion,
}

impl FirmwareFeature {
    fn cargo_env_name(&self) -> String {
        format!("CARGO_FEATURE_{}", self.name.to_ascii_uppercase())
    }

    fn cfg_name(&self) -> String {
        format!("fw_{}", self.version.cfg_suffix())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FirmwareVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl FirmwareVersion {
    fn parse_feature_name(name: &str) -> Option<Self> {
        let version = name.strip_prefix(FIRMWARE_FEATURE_PREFIX)?;
        let parts = version.split('_').collect::<Vec<_>>();
        let [major, minor, patch] = parts.as_slice() else {
            return None;
        };

        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            patch: patch.parse().ok()?,
        })
    }

    fn cfg_suffix(&self) -> String {
        format!("v{}_{}_{}", self.major, self.minor, self.patch)
    }
}
