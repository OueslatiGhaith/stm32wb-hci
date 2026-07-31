use std::{
    cmp::Ordering,
    env,
    path::{Path, PathBuf},
};

use stm32wb_hci_schema::FirmwareVersion;

fn main() {
    println!("cargo::rerun-if-changed=Cargo.toml");

    let manifest_path = manifest_path();
    let crate_dir = manifest_path
        .parent()
        .expect("Cargo.toml path must have a parent directory");
    let mut firmwares = FirmwareVersion::declared_in_manifest(crate_dir).unwrap_or_else(|error| {
        panic!(
            "failed to discover firmware features from {}: {error}",
            manifest_path.display()
        )
    });

    if firmwares.is_empty() {
        panic!("no `fw_<major>_<minor>_<patch>` features were found in [features]");
    }

    // `FirmwareVersion` has numeric ordering, so 1.17.0 correctly sorts after
    // 1.9.0. The same ordering is used by the compliance checker when it
    // evaluates a source-level `before_fw_`/`only_fw_`/`since_fw_` predicate.
    firmwares.sort();
    for pair in firmwares.windows(2) {
        if pair[0] == pair[1] {
            panic!(
                "firmware feature `{}` is declared more than once",
                pair[0].feature_name()
            );
        }
    }

    for firmware in &firmwares {
        let feature = firmware.feature_name();
        println!("cargo::rerun-if-env-changed={}", feature_env_var(&feature));
        for prefix in ["before", "only", "since"] {
            println!("cargo::rustc-check-cfg=cfg({prefix}_{feature})");
        }
    }

    let enabled = firmwares
        .iter()
        .filter(|firmware| env::var_os(feature_env_var(&firmware.feature_name())).is_some())
        .collect::<Vec<_>>();

    let [selected] = enabled.as_slice() else {
        let enabled = enabled
            .iter()
            .map(|firmware| firmware.feature_name())
            .collect::<Vec<_>>()
            .join(", ");
        let available = firmwares
            .iter()
            .map(|firmware| firmware.feature_name())
            .collect::<Vec<_>>()
            .join(", ");

        panic!(
            "exactly one firmware feature must be enabled; enabled: [{enabled}]; available: [{available}]"
        );
    };
    let selected = *selected;

    for firmware in &firmwares {
        let feature = firmware.feature_name();
        match selected.cmp(firmware) {
            Ordering::Less => emit_cfg("before", &feature),
            Ordering::Equal => emit_cfg("only", &feature),
            Ordering::Greater => {}
        }

        if selected >= firmware {
            emit_cfg("since", &feature);
        }
    }
}

fn manifest_path() -> PathBuf {
    Path::new(
        &env::var_os("CARGO_MANIFEST_DIR")
            .expect("Cargo must set CARGO_MANIFEST_DIR for build scripts"),
    )
    .join("Cargo.toml")
}

fn feature_env_var(feature: &str) -> String {
    format!(
        "CARGO_FEATURE_{}",
        feature.to_ascii_uppercase().replace('-', "_")
    )
}

fn emit_cfg(prefix: &str, feature: &str) {
    println!("cargo::rustc-cfg={prefix}_{feature}");
}
