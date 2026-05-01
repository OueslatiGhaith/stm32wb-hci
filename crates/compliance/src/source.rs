use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

const AUTO_DIR: &str = "Middlewares/ST/STM32_WPAN/ble/core/auto";

pub struct CubeSource {
    root: PathBuf,
    tag: Option<String>,
}

impl CubeSource {
    pub fn new(root: impl Into<PathBuf>, tag: Option<String>) -> Self {
        Self {
            root: root.into(),
            tag,
        }
    }

    pub fn firmware_label(&self) -> String {
        self.tag
            .clone()
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub fn load_auto_file(&self, name: &str) -> Result<String> {
        let path = format!("{AUTO_DIR}/{name}");

        if let Some(tag) = &self.tag {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .arg("show")
                .arg(format!("{tag}:{path}"))
                .output()
                .with_context(|| format!("failed to run git show for {path}"))?;

            if !output.status.success() {
                bail!(
                    "git show failed for {tag}:{path}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }

            return String::from_utf8(output.stdout)
                .with_context(|| format!("{path} is not valid UTF-8"));
        }

        let full_path = self.root.join(Path::new(&path));
        std::fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read {}", full_path.display()))
    }
}
