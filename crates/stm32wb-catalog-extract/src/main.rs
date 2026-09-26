//! Extract the STM32WB wireless-interface catalog from tagged STM32CubeWB
//! releases, or verify that the checked-in catalog is reproducible.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use stm32wb_catalog::Catalog;
use stm32wb_catalog::annotations::Annotations;

#[derive(Parser)]
#[command(about = "Extract the STM32WB wireless-interface catalog from STM32CubeWB tags")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the catalog and audit its annotations without a Cube clone.
    Audit {
        #[command(flatten)]
        paths: Paths,
    },
}

#[derive(Args)]
struct Paths {
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

    fn catalog(&self) -> PathBuf {
        self.catalog
            .clone()
            .unwrap_or_else(|| Self::workspace().join("catalog/stm32wb.toml"))
    }

    fn annotations(&self) -> PathBuf {
        self.annotations
            .clone()
            .unwrap_or_else(|| Self::workspace().join("catalog/annotations.toml"))
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
        Command::Audit { paths } => {
            let catalog = Catalog::load(&paths.catalog()).map_err(|error| error.to_string())?;
            audit(&catalog, &paths.annotations())?;
            eprintln!("catalog and annotations are consistent");
            Ok(())
        }
    }
}

fn audit(catalog: &Catalog, annotations: &Path) -> Result<(), String> {
    if !annotations.exists() {
        return Ok(());
    }
    Annotations::load(annotations)
        .and_then(|annotations| annotations.audit(catalog))
        .map_err(|error| error.to_string())
}
