use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use super::gerber::classify_pcb_file;
use super::types::{Artifact, DomainError, DomainResult};

const DEFAULT_MAX_ENTRIES: usize = 10_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, Serialize)]
pub struct PackageLimits {
    pub max_entries: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_compression_ratio: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Directory,
    Zip,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct PackageInventory {
    pub source: String,
    pub kind: PackageKind,
    pub files: Vec<Artifact>,
}

pub fn inspect_package(
    source: impl AsRef<Path>,
    limits: PackageLimits,
) -> DomainResult<PackageInventory> {
    let source = source.as_ref();
    let metadata = fs::metadata(source)?;
    if metadata.is_dir() {
        inspect_directory(source, limits)
    } else if metadata.is_file() {
        inspect_zip(source, limits)
    } else {
        Err(DomainError::InvalidInput(format!(
            "source is neither a directory nor a regular file: {}",
            source.display()
        )))
    }
}

pub fn read_package_member(
    source: impl AsRef<Path>,
    member: &str,
    limits: PackageLimits,
) -> DomainResult<Vec<u8>> {
    validate_member_path(member)?;
    let source = source.as_ref();
    if fs::metadata(source)?.is_dir() {
        let root = fs::canonicalize(source)?;
        let requested = root.join(member);
        let target = fs::canonicalize(&requested)?;
        if !target.starts_with(&root) || !fs::metadata(&target)?.is_file() {
            return Err(DomainError::InvalidInput(format!(
                "member is not a regular file within package: {member}"
            )));
        }
        read_limited(File::open(target)?, limits.max_file_bytes)
    } else {
        let mut archive = open_zip(source)?;
        let mut entry = archive.by_name(member)?;
        if !entry.is_file() {
            return Err(DomainError::InvalidInput(format!(
                "member is not a regular file: {member}"
            )));
        }
        validate_zip_entry(&entry, limits)?;
        read_limited(&mut entry, limits.max_file_bytes)
    }
}

fn inspect_directory(source: &Path, limits: PackageLimits) -> DomainResult<PackageInventory> {
    let root = fs::canonicalize(source)?;
    let mut paths = Vec::new();
    collect_directory_files(&root, &root, &mut paths, limits)?;
    paths.sort();

    let mut total = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let metadata = fs::metadata(&path)?;
        total = total.checked_add(metadata.len()).ok_or_else(|| {
            DomainError::LimitExceeded("directory package total size overflow".to_owned())
        })?;
        if total > limits.max_total_bytes {
            return Err(DomainError::LimitExceeded(format!(
                "directory package exceeds {} bytes",
                limits.max_total_bytes
            )));
        }
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| DomainError::InvalidInput("package path escaped its root".to_owned()))?;
        files.push(Artifact {
            path: path_to_slash(relative),
            role: classify_pcb_file(&path_to_slash(relative))
                .as_str()
                .to_owned(),
            bytes: metadata.len(),
            sha256: hash_reader(File::open(&path)?)?,
            parser_status: None,
        });
    }
    Ok(PackageInventory {
        source: source.display().to_string(),
        kind: PackageKind::Directory,
        files,
    })
}

fn collect_directory_files(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
    limits: PackageLimits,
) -> DomainResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_symlink() {
            return Err(DomainError::InvalidInput(format!(
                "symbolic links are not allowed in a release package: {}",
                path.strip_prefix(root).unwrap_or(&path).display()
            )));
        }
        if file_type.is_dir() {
            collect_directory_files(root, &path, paths, limits)?;
        } else if file_type.is_file() {
            if paths.len() >= limits.max_entries {
                return Err(DomainError::LimitExceeded(format!(
                    "package has more than {} files",
                    limits.max_entries
                )));
            }
            if entry.metadata()?.len() > limits.max_file_bytes {
                return Err(DomainError::LimitExceeded(format!(
                    "file exceeds {} bytes: {}",
                    limits.max_file_bytes,
                    path.display()
                )));
            }
            paths.push(path);
        }
    }
    Ok(())
}

fn inspect_zip(source: &Path, limits: PackageLimits) -> DomainResult<PackageInventory> {
    let mut archive = open_zip(source)?;
    if archive.len() > limits.max_entries {
        return Err(DomainError::LimitExceeded(format!(
            "ZIP has more than {} entries",
            limits.max_entries
        )));
    }

    let mut total = 0_u64;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        validate_member_path(&name)?;
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            return Err(DomainError::InvalidInput(format!(
                "symbolic links are not allowed in a ZIP release package: {name}"
            )));
        }
        validate_zip_entry(&entry, limits)?;
        total = total.checked_add(entry.size()).ok_or_else(|| {
            DomainError::LimitExceeded("ZIP package total size overflow".to_owned())
        })?;
        if total > limits.max_total_bytes {
            return Err(DomainError::LimitExceeded(format!(
                "ZIP package exceeds {} bytes",
                limits.max_total_bytes
            )));
        }
        files.push(Artifact {
            path: name,
            role: classify_pcb_file(entry.name()).as_str().to_owned(),
            bytes: entry.size(),
            sha256: hash_reader(&mut entry)?,
            parser_status: None,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(PackageInventory {
        source: source.display().to_string(),
        kind: PackageKind::Zip,
        files,
    })
}

fn open_zip(source: &Path) -> DomainResult<ZipArchive<File>> {
    Ok(ZipArchive::new(File::open(source)?)?)
}

fn validate_zip_entry(entry: &zip::read::ZipFile<'_>, limits: PackageLimits) -> DomainResult<()> {
    if entry.size() > limits.max_file_bytes {
        return Err(DomainError::LimitExceeded(format!(
            "ZIP entry exceeds {} bytes: {}",
            limits.max_file_bytes,
            entry.name()
        )));
    }
    let compressed = entry.compressed_size();
    if entry.size() > 0
        && compressed > 0
        && entry.size() / compressed > limits.max_compression_ratio
    {
        return Err(DomainError::LimitExceeded(format!(
            "ZIP entry compression ratio exceeds {}: {}",
            limits.max_compression_ratio,
            entry.name()
        )));
    }
    Ok(())
}

fn validate_member_path(value: &str) -> DomainResult<()> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(DomainError::InvalidInput(format!(
            "unsafe package member path: {value}"
        )));
    }
    Ok(())
}

fn read_limited(reader: impl Read, max_bytes: u64) -> DomainResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(DomainError::LimitExceeded(format!(
            "member exceeds {} bytes",
            max_bytes
        )));
    }
    Ok(bytes)
}

fn hash_reader(mut reader: impl Read) -> DomainResult<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
