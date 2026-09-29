//! Read-only access to immutable tags of an STM32CubeWB or STM32CubeWBA Git
//! repository, and of the submodules they pin.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use stm32wb_catalog::Version;
use tempfile::TempDir;

pub const BLE_CORE_DIR: &str = "Middlewares/ST/STM32_WPAN/ble/core";
pub const SHCI_DIR: &str = "Middlewares/ST/STM32_WPAN/interface/patterns/ble_thread";
pub const INTERFACE_DOCUMENT: &str =
    "Middlewares/ST/STM32_WPAN/ble/core/doc/STM32WB_BLE_Wireless_Interface.html";
pub const BINARIES_DIR: &str = "Projects/STM32WB_Copro_Wireless_Binaries";

/// One tag of a local Cube clone, or the commit a tag pins a submodule at.
/// Files are read from Git objects, so the clone's worktree is never
/// consulted or modified.
pub struct CubeTag {
    repository: PathBuf,
    pub tag: String,
    pub commit: String,
}

impl CubeTag {
    pub fn open(repository: &Path, version: Version) -> Result<Self, String> {
        let tag = version.cube_tag();
        let commit = git_text(
            repository,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/tags/{tag}^{{commit}}"),
            ],
        )
        .map_err(|error| {
            format!(
                "tag {tag} was not found in {}: {error}",
                repository.display()
            )
        })?
        .trim()
        .to_owned();
        if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "tag {tag} resolved to an invalid commit {commit:?}"
            ));
        }
        Ok(Self {
            repository: repository.to_owned(),
            tag,
            commit,
        })
    }

    /// The directory `path` of this tag: `(tag, prefix)` to read it with,
    /// the prefix ending in `/`. A directory the tag pins as a submodule is
    /// read from the submodule's clone at `path` in the worktree, at the
    /// pinned commit.
    pub fn directory(&self, path: &str) -> Result<(Self, String), String> {
        let entry = git_text(&self.repository, &["ls-tree", &self.commit, "--", path])?;
        let fields = entry.split_whitespace().collect::<Vec<_>>();
        match fields[..] {
            ["040000", "tree", _, name] if name == path => Ok((
                Self {
                    repository: self.repository.clone(),
                    tag: self.tag.clone(),
                    commit: self.commit.clone(),
                },
                format!("{path}/"),
            )),
            ["160000", "commit", commit, name] if name == path => {
                let repository = self.repository.join(path);
                git_text(
                    &repository,
                    &["cat-file", "-e", &format!("{commit}^{{commit}}")],
                )
                .map_err(|error| {
                    format!(
                        "{} pins {path} at {commit}, which {} does not have; run \
                         `git submodule update --init {path}` in {}: {error}",
                        self.tag,
                        repository.display(),
                        self.repository.display()
                    )
                })?;
                Ok((
                    Self {
                        repository,
                        tag: format!("{} {path}", self.tag),
                        commit: commit.to_owned(),
                    },
                    String::new(),
                ))
            }
            _ => Err(format!("{} has no directory {path}: {entry:?}", self.tag)),
        }
    }

    /// Read one file as bytes. The commit, not the tag name, is used so the
    /// result is stable even if a tag were moved during extraction.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        git_bytes(
            &self.repository,
            &["show", &format!("{}:{path}", self.commit)],
        )
    }

    pub fn read_text(&self, path: &str) -> Result<String, String> {
        let bytes = self.read(path)?;
        // ST occasionally ships Latin-1 comments; the protocol content is ASCII.
        Ok(String::from_utf8(bytes).unwrap_or_else(|error| {
            error
                .into_bytes()
                .iter()
                .map(|&byte| char::from(byte))
                .collect()
        }))
    }

    pub fn list(&self, directory: &str) -> Result<Vec<String>, String> {
        let listing = git_text(
            &self.repository,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                &self.commit,
                "--",
                directory,
            ],
        )?;
        Ok(listing.lines().map(str::to_owned).collect())
    }

    /// Write the listed directories into a fresh temporary tree, preserving
    /// their repository-relative paths.
    pub fn materialize(&self, directories: &[&str]) -> Result<TempDir, String> {
        let temporary = tempfile::tempdir()
            .map_err(|error| format!("could not create a temporary tree: {error}"))?;
        for directory in directories {
            let files = self.list(directory)?;
            if files.is_empty() {
                return Err(format!("{} contains no files at {}", directory, self.tag));
            }
            for file in files {
                let destination = checked_join(temporary.path(), &file)?;
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        format!("could not create {}: {error}", parent.display())
                    })?;
                }
                fs::write(&destination, self.read(&file)?).map_err(|error| {
                    format!("could not write {}: {error}", destination.display())
                })?;
            }
        }
        Ok(temporary)
    }
}

fn checked_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(root.join(path))
    } else {
        Err(format!(
            "refusing to materialize unexpected path {relative:?}"
        ))
    }
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn git_text(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    String::from_utf8(git_bytes(repository, arguments)?).map_err(|error| {
        format!(
            "git {} returned non-UTF-8 output: {error}",
            arguments.join(" ")
        )
    })
}
