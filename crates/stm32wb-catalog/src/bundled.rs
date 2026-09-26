//! The catalog and annotations shipped inside this crate.
//!
//! The files are embedded at compile time, so any crate depending on this one
//! (in particular the proc macros that check declarations against the
//! catalog) is rebuilt whenever the extractor rewrites them.

use std::sync::OnceLock;

use crate::annotations::Annotations;
use crate::{Catalog, Error};

const CATALOG: &str = include_str!("../catalog/stm32wb.toml");
const ANNOTATIONS: &str = include_str!("../catalog/annotations.toml");

/// The bundled catalog together with the annotations audited against it.
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

/// Parse, validate, and audit the bundled files once per process.
pub fn bundled() -> Result<&'static Bundled, Error> {
    static BUNDLED: OnceLock<Result<Bundled, Error>> = OnceLock::new();
    BUNDLED
        .get_or_init(|| {
            let catalog = Catalog::from_toml(CATALOG)
                .map_err(|error| error.context("bundled stm32wb.toml"))?;
            let annotations = Annotations::from_toml(ANNOTATIONS)
                .map_err(|error| error.context("bundled annotations.toml"))?;
            Bundled::new(catalog, annotations)
        })
        .as_ref()
        .map_err(Clone::clone)
}
