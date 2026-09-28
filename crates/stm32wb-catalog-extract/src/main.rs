//! Extract the STM32WB wireless-interface catalog from tagged STM32CubeWB
//! releases, or verify that the checked-in catalog is reproducible.

mod c;
mod commands;
mod cube;
mod docs;
mod domains;
mod events;
mod shci;
mod snapshot;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clang::{Clang, Index};
use clap::{Args, Parser, Subcommand};
use stm32wb_catalog::annotations::Annotations;
use stm32wb_catalog::{Bundled, Catalog, Platform, Version, merge_snapshots};

#[derive(Parser)]
#[command(about = "Extract the STM32WB wireless-interface catalog from STM32CubeWB tags")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Re-extract the catalog and write it.
    Extract {
        #[command(flatten)]
        paths: Paths,
        /// Releases to extract (default: the releases the catalog describes).
        #[arg(long = "release", value_name = "VERSION")]
        releases: Vec<Version>,
        /// Add releases to those the catalog already describes.
        #[arg(long = "add", value_name = "VERSION", conflicts_with = "releases")]
        additions: Vec<Version>,
    },
    /// Re-extract the catalog and fail if it differs from the checked-in file.
    Check {
        #[command(flatten)]
        paths: Paths,
    },
    /// Validate the catalog and audit its annotations without a Cube clone.
    Audit {
        #[command(flatten)]
        paths: Paths,
    },
    /// Print the Cargo features of one target per distinct interface, one per
    /// line, so CI can build every distinct configuration of stm32wb-hci.
    Targets {
        #[command(flatten)]
        paths: Paths,
    },
}

#[derive(Args)]
struct Paths {
    /// Local STM32CubeWB clone with its release tags.
    #[arg(long, value_name = "PATH")]
    cube: Option<PathBuf>,
    /// The checked-in catalog.
    #[arg(long, value_name = "PATH")]
    catalog: Option<PathBuf>,
    /// The checked-in annotations.
    #[arg(long, value_name = "PATH")]
    annotations: Option<PathBuf>,
}

impl Paths {
    fn workspace() -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        workspace.canonicalize().unwrap_or(workspace)
    }

    fn cube(&self) -> PathBuf {
        self.cube
            .clone()
            .unwrap_or_else(|| Self::workspace().join("STM32CubeWB"))
    }

    /// Where `stm32wb-catalog` bundles the files this tool maintains.
    fn bundled() -> PathBuf {
        Self::workspace().join("crates/stm32wb-catalog/catalog")
    }

    fn catalog(&self) -> PathBuf {
        self.catalog
            .clone()
            .unwrap_or_else(|| Self::bundled().join("stm32wb.toml"))
    }

    fn annotations(&self) -> PathBuf {
        self.annotations
            .clone()
            .unwrap_or_else(|| Self::bundled().join("annotations.toml"))
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Extract {
            paths,
            releases,
            additions,
        } => {
            let mut releases = if releases.is_empty() {
                existing_releases(&paths.catalog())?
            } else {
                releases
            };
            releases.extend(additions);
            releases.sort();
            releases.dedup();
            if releases.is_empty() {
                return Err("no releases to extract; pass --release".to_owned());
            }
            let catalog = extract(&paths.cube(), &releases)?;
            let text = render(&catalog)?;
            fs::write(paths.catalog(), text).map_err(|error| {
                format!("could not write {}: {error}", paths.catalog().display())
            })?;
            audit(&catalog, &paths.annotations())?;
            eprintln!("wrote {}", paths.catalog().display());
            Ok(())
        }
        Command::Check { paths } => {
            let checked_in = fs::read_to_string(paths.catalog()).map_err(|error| {
                format!("could not read {}: {error}", paths.catalog().display())
            })?;
            let releases = existing_releases(&paths.catalog())?;
            let catalog = extract(&paths.cube(), &releases)?;
            if render(&catalog)? != checked_in {
                return Err(format!(
                    "{} is not what the tagged Cube sources produce; run `extract` and review the diff",
                    paths.catalog().display()
                ));
            }
            audit(&catalog, &paths.annotations())?;
            eprintln!("{} is reproducible", paths.catalog().display());
            Ok(())
        }
        Command::Audit { paths } => {
            let catalog = Catalog::load(&paths.catalog()).map_err(|error| error.to_string())?;
            audit(&catalog, &paths.annotations())?;
            eprintln!("catalog and annotations are consistent");
            Ok(())
        }
        Command::Targets { paths } => {
            let catalog = Catalog::load(&paths.catalog()).map_err(|error| error.to_string())?;
            let annotations =
                Annotations::load(&paths.annotations()).map_err(|error| error.to_string())?;
            let bundled = Bundled::new(catalog, annotations).map_err(|error| error.to_string())?;
            for target in bundled.distinct_targets() {
                println!("{}", target.features());
            }
            Ok(())
        }
    }
}

fn existing_releases(path: &Path) -> Result<Vec<Version>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let catalog = Catalog::load(path).map_err(|error| error.to_string())?;
    Ok(catalog.versions().collect())
}

fn extract(cube: &Path, releases: &[Version]) -> Result<Catalog, String> {
    let clang = Clang::new().map_err(|error| format!("could not load libclang: {error}"))?;
    let index = Index::new(&clang, false, false);
    let shim = c::Shim::new()?;
    let mut snapshots = Vec::new();
    for release in releases {
        let (snapshot, report) = snapshot::extract(&index, &shim, cube, *release)?;
        eprintln!(
            "{}: {} commands, {} events, {} count relations proven by code, {} unresolved layouts",
            report.version,
            report.commands,
            report.events,
            report.proven_counts,
            report.unresolved.len()
        );
        for (name, layout, reason) in &report.unresolved {
            eprintln!("  unresolved {name} {layout}: {reason}");
        }
        eprintln!("  {} documented value lists", report.domains);
        for (name, member) in &report.dropped_domains {
            eprintln!("  dropped the values of {name} {member}: its layout is unresolved");
        }
        snapshots.push(snapshot);
    }
    merge_snapshots(Platform::Stm32wb, snapshots).map_err(|error| error.to_string())
}

const HEADER: &str = "\
# The STM32WB wireless-interface catalog, layers 1 and 2.
#
# Generated by `cargo run -p stm32wb-catalog-extract -- extract`; do not edit.
# Every fact is read from the tagged STM32CubeWB sources listed under
# [[releases]], and `-- check` verifies that this file is reproducible.
# Curated facts belong in annotations.toml.

";

fn render(catalog: &Catalog) -> Result<String, String> {
    Ok(format!(
        "{HEADER}{}",
        catalog.to_toml().map_err(|error| error.to_string())?
    ))
}

fn audit(catalog: &Catalog, annotations: &Path) -> Result<(), String> {
    if !annotations.exists() {
        return Ok(());
    }
    Annotations::load(annotations)
        .and_then(|annotations| annotations.audit(catalog))
        .map_err(|error| error.to_string())
}
