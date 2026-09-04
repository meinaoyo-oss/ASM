use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub const DEFAULT_MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_MAX_ARCHIVE_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub filesystem: FilesystemConfig,
    pub kicad: KicadConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct FilesystemConfig {
    pub allowed_roots: Vec<PathBuf>,
    pub max_file_bytes: u64,
    pub max_archive_uncompressed_bytes: u64,
    pub max_archive_entries: usize,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            allowed_roots: Vec::new(),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_archive_uncompressed_bytes: DEFAULT_MAX_ARCHIVE_BYTES,
            max_archive_entries: DEFAULT_MAX_ARCHIVE_ENTRIES,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct KicadConfig {
    pub cli_path: Option<PathBuf>,
    pub timeout_seconds: u64,
}

impl Default for KicadConfig {
    fn default() -> Self {
        Self {
            cli_path: None,
            timeout_seconds: 120,
        }
    }
}

impl AppConfig {
    pub fn load(path: Option<&Path>, cli_roots: &[PathBuf]) -> Result<Self> {
        let mut config = if let Some(path) = path {
            let text = fs::read_to_string(path)
                .with_context(|| format!("无法读取配置文件 {}", path.display()))?;
            toml::from_str(&text).with_context(|| format!("无法解析配置文件 {}", path.display()))?
        } else {
            Self::default()
        };

        if !cli_roots.is_empty() {
            config.filesystem.allowed_roots = cli_roots.to_vec();
        }
        if config.filesystem.allowed_roots.is_empty() {
            config.filesystem.allowed_roots.push(
                env::current_dir()
                    .context("无法读取当前工作目录")?
                    .canonicalize()
                    .context("无法解析当前工作目录")?,
            );
        } else {
            config.filesystem.allowed_roots = config
                .filesystem
                .allowed_roots
                .iter()
                .map(|path| {
                    path.canonicalize()
                        .with_context(|| format!("允许根目录不存在: {}", path.display()))
                })
                .collect::<Result<Vec<_>>>()?;
        }

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.filesystem.max_file_bytes == 0
            || self.filesystem.max_archive_uncompressed_bytes == 0
            || self.filesystem.max_archive_entries == 0
        {
            bail!("文件和压缩包限制必须大于零");
        }
        if self.kicad.timeout_seconds == 0 {
            bail!("KiCad 超时必须大于零");
        }
        Ok(())
    }

    pub fn resolve_input(&self, path: &Path) -> Result<PathBuf> {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("输入路径不存在: {}", path.display()))?;
        if self.path_is_allowed(&canonical) {
            Ok(canonical)
        } else {
            bail!("输入路径不在允许根目录内: {}", path.display())
        }
    }

    pub fn resolve_output_directory(&self, path: &Path) -> Result<PathBuf> {
        let candidate = if path.exists() {
            path.canonicalize()
                .with_context(|| format!("无法解析报告目录: {}", path.display()))?
        } else {
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
            {
                bail!("报告目录不能包含 ..: {}", path.display());
            }
            let absolute = if path.is_absolute() {
                path.to_owned()
            } else {
                env::current_dir()?.join(path)
            };
            let mut existing = absolute.as_path();
            while !existing.exists() {
                existing = existing.parent().context("报告目录没有存在的祖先目录")?;
            }
            let relative = absolute
                .strip_prefix(existing)
                .context("无法解析报告目录的相对部分")?;
            existing
                .canonicalize()
                .with_context(|| format!("无法解析报告目录祖先: {}", existing.display()))?
                .join(relative)
        };
        if self.path_is_allowed(&candidate) {
            Ok(candidate)
        } else {
            bail!("报告目录不在允许根目录内: {}", path.display())
        }
    }

    pub fn path_is_allowed(&self, path: &Path) -> bool {
        self.filesystem
            .allowed_roots
            .iter()
            .any(|root| path.starts_with(root))
    }

    pub fn find_kicad_cli(&self) -> Option<PathBuf> {
        if let Some(path) = &self.kicad.cli_path {
            return path.is_file().then(|| path.clone());
        }
        find_on_path(if cfg!(windows) {
            "kicad-cli.exe"
        } else {
            "kicad-cli"
        })
    }
}

fn find_on_path(command: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(command))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricts_inputs_and_reports_to_allowed_roots() {
        let root = tempfile::tempdir().expect("temporary root");
        let outside = tempfile::tempdir().expect("outside root");
        let input = root.path().join("board.zip");
        fs::write(&input, b"fixture").expect("write fixture");
        let outside_input = outside.path().join("board.zip");
        fs::write(&outside_input, b"fixture").expect("write outside fixture");

        let config = AppConfig::load(None, &[root.path().to_owned()]).expect("valid config");
        assert_eq!(config.resolve_input(&input).expect("allowed input"), input);
        assert!(config.resolve_input(&outside_input).is_err());
        assert!(
            config
                .resolve_output_directory(&root.path().join("reports/run-1"))
                .is_ok()
        );
        assert!(
            config
                .resolve_output_directory(&outside.path().join("reports"))
                .is_err()
        );
    }
}
