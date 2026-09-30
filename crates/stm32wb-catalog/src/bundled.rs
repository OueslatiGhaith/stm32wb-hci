//! The catalogs and annotations shipped inside this crate.
//!
//! The files are embedded at compile time, so any crate depending on this one
//! (in particular the proc macros that check declarations against the
//! catalogs) is rebuilt whenever the extractor rewrites them.

use std::sync::OnceLock;

use crate::annotations::Annotations;
use crate::{Catalog, Error, Platform};

/// The bundled catalog of a platform together with the annotations audited
/// against it.
#[derive(Debug)]
pub struct Bundled {
    pub catalog: Catalog,
    pub annotations: Annotations,
}

impl Bundled {
    /// Pair a catalog with annotations after auditing them against it.
    pub fn new(catalog: Catalog, annotations: Annotations) -> Result<Self, Error> {
        annotations.audit(&catalog)?;
        Ok(Self {
            catalog,
            annotations,
        })
    }
}

/// Parse, validate, and audit the bundled files of `platform` once per
/// process.
pub fn bundled(platform: Platform) -> Result<&'static Bundled, Error> {
    static STM32WB: OnceLock<Result<Bundled, Error>> = OnceLock::new();
    static STM32WBA: OnceLock<Result<Bundled, Error>> = OnceLock::new();
    let (cell, files) = match platform {
        Platform::Stm32wb => (
            &STM32WB,
            [
                ("stm32wb.toml", include_str!("../catalog/stm32wb.toml")),
                (
                    "annotations.toml",
                    include_str!("../catalog/annotations.toml"),
                ),
            ],
        ),
        Platform::Stm32wba => (
            &STM32WBA,
            [
                ("stm32wba.toml", include_str!("../catalog/stm32wba.toml")),
                (
                    "stm32wba-annotations.toml",
                    include_str!("../catalog/stm32wba-annotations.toml"),
                ),
            ],
        ),
    };
    let [(catalog_name, catalog), (annotations_name, annotations)] = files;
    cell.get_or_init(|| {
        let catalog = Catalog::from_toml(catalog)
            .map_err(|error| error.context(format!("bundled {catalog_name}")))?;
        let annotations = Annotations::from_toml(annotations)
            .map_err(|error| error.context(format!("bundled {annotations_name}")))?;
        let bundled = Bundled::new(catalog, annotations)?;
        if bundled.catalog.platform != platform {
            return Err(Error::invalid(format!(
                "bundled {catalog_name} describes {:?}",
                bundled.catalog.platform
            )));
        }
        Ok(bundled)
    })
    .as_ref()
    .map_err(Clone::clone)
}
