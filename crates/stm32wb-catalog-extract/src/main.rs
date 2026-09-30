//! Extract the STM32WB or STM32WBA wireless-interface catalog from tagged
//! STM32CubeWB or STM32CubeWBA releases, or verify that the checked-in catalog
//! is reproducible.

mod c;
mod commands;
mod cube;
mod declared;
mod docs;
mod domains;
mod events;
mod shci;
mod snapshot;
mod statuses;
#[cfg(test)]
mod tests;
mod wba;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clang::{Clang, Index};
use clap::{Args, Parser, Subcommand, ValueEnum};
use stm32wb_catalog::annotations::Annotations;
use stm32wb_catalog::{Bundled, Catalog, Platform, Version, merge_snapshots};

#[derive(Parser)]
#[command(
    about = "Extract the STM32WB and STM32WBA wireless-interface catalogs from STM32Cube tags"
)]
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

/// The platform a catalog describes.
#[derive(Clone, Copy, Default, ValueEnum)]
enum PlatformArg {
    #[default]
    Stm32wb,
    Stm32wba,
}

impl PlatformArg {
    fn platform(self) -> Platform {
        match self {
            Self::Stm32wb => Platform::Stm32wb,
            Self::Stm32wba => Platform::Stm32wba,
        }
    }

    /// The STM32Cube package, which names the default clone.
    fn cube(self) -> &'static str {
        self.platform().package()
    }

    fn catalog(self) -> &'static str {
        match self {
            Self::Stm32wb => "stm32wb.toml",
            Self::Stm32wba => "stm32wba.toml",
        }
    }

    fn annotations(self) -> &'static str {
        match self {
            Self::Stm32wb => "annotations.toml",
            Self::Stm32wba => "stm32wba-annotations.toml",
        }
    }
}

#[derive(Args)]
struct Paths {
    /// The platform whose catalog to maintain.
    #[arg(long, value_enum, default_value = "stm32wb")]
    platform: PlatformArg,
    /// Local STM32CubeWB or STM32CubeWBA clone with its release tags.
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
            .unwrap_or_else(|| Self::workspace().join(self.platform.cube()))
    }

    /// Where `stm32wb-catalog` bundles the files this tool maintains.
    fn bundled() -> PathBuf {
        Self::workspace().join("crates/stm32wb-catalog/catalog")
    }

    fn catalog(&self) -> PathBuf {
        self.catalog
            .clone()
            .unwrap_or_else(|| Self::bundled().join(self.platform.catalog()))
    }

    fn annotations(&self) -> PathBuf {
        self.annotations
            .clone()
            .unwrap_or_else(|| Self::bundled().join(self.platform.annotations()))
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
            let catalog = extract(paths.platform, &paths.cube(), &releases)?;
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
            let catalog = extract(paths.platform, &paths.cube(), &releases)?;
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

fn extract(platform: PlatformArg, cube: &Path, releases: &[Version]) -> Result<Catalog, String> {
    let clang = Clang::new().map_err(|error| format!("could not load libclang: {error}"))?;
    let index = Index::new(&clang, false, false);
    let shim = c::Shim::new()?;
    let mut snapshots = Vec::new();
    for release in releases {
        let (snapshot, report) = match platform {
            PlatformArg::Stm32wb => snapshot::extract(&index, &shim, cube, *release)?,
            PlatformArg::Stm32wba => wba::extract(&index, &shim, cube, *release)?,
        };
        eprintln!(
            "{}: {} commands, {} events, {} unresolved layouts",
            report.version,
            report.commands,
            report.events,
            report.unresolved.len()
        );
        match platform {
            PlatformArg::Stm32wb => {
                eprintln!(
                    "  {} count relations proven by code, {} layouts reproduced from their structures alone, {} commands and events not",
                    report.proven_counts,
                    report.declared_agreements,
                    report.declared_differences.len()
                );
                for (name, difference) in &report.declared_differences {
                    eprintln!("  {name} is not reproduced from its structures: {difference}");
                }
                eprintln!(
                    "  {} command completions stated by the interface document, as the code has them",
                    report.documented_completions
                );
                for name in &report.unstated_completions {
                    eprintln!("  the interface document does not state how {name} completes");
                }
            }
            PlatformArg::Stm32wba => {
                eprintln!(
                    "  {} command completions stated by the interface document",
                    report.documented_completions
                );
                for name in &report.unstated_completions {
                    eprintln!(
                        "  left out {name}: the interface document does not state how it completes"
                    );
                }
                for name in &report.undeclared {
                    eprintln!(
                        "  left out {name}: the interface document lists it, but no header declares it"
                    );
                }
                for name in &report.undocumented {
                    eprintln!(
                        "  left out {name}: a header declares it, but the interface document does not list it"
                    );
                }
            }
        }
        for (name, layout, reason) in &report.unresolved {
            eprintln!("  unresolved {name} {layout}: {reason}");
        }
        eprintln!("  {} documented value lists", report.domains);
        for (name, member) in &report.deferred_domains {
            eprintln!(
                "  kept the values of {name} {member} for the annotation of its unresolved layout"
            );
        }
        for (name, member) in &report.dropped_domains {
            eprintln!("  dropped the values of {name} {member}: it has no layout on that side");
        }
        for (name, member) in &report.dropped_flags {
            eprintln!("  dropped the flags of {name} {member}: an item is not a single bit");
        }
        for (structure, member) in &report.dropped_field_domains {
            eprintln!(
                "  dropped the values of {structure}.{member}: no resolved layout carries {structure}"
            );
        }
        for (name, header) in &report.orphaned_lists {
            eprintln!("  dropped a list of {name} outside any @param block: {header:?}");
        }
        for (name, member) in &report.deferred_bearers {
            eprintln!(
                "  kept the bearer {name} {member} for the annotation of its unresolved layout"
            );
        }
        snapshots.push(snapshot);
    }
    merge_snapshots(platform.platform(), snapshots).map_err(|error| error.to_string())
}

fn render(catalog: &Catalog) -> Result<String, String> {
    let platform = match catalog.platform {
        Platform::Stm32wb => PlatformArg::Stm32wb,
        Platform::Stm32wba => PlatformArg::Stm32wba,
    };
    let chip = platform.cube().trim_start_matches("STM32Cube");
    let arguments = match platform {
        PlatformArg::Stm32wb => "",
        PlatformArg::Stm32wba => " --platform stm32wba",
    };
    let annotations = platform.annotations();
    Ok(format!(
        "\
# The STM32{chip} wireless-interface catalog, layers 1 and 2.
#
# Generated by `cargo run -p stm32wb-catalog-extract -- extract{arguments}`; do not edit.
# Every fact is read from the tagged STM32Cube{chip} sources listed under
# [[releases]], and `-- check{arguments}` verifies that this file is reproducible.
# Curated facts belong in {annotations}.

{}",
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
