use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{NeroError, Result, Workspace};

const MANIFEST_NAME: &str = "manifest.json";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub created_at_unix: u64,
    pub files: Vec<BackupEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStats {
    pub files: usize,
    pub bytes: u64,
}

impl BackupManifest {
    pub fn stats(&self) -> BackupStats {
        BackupStats {
            files: self.files.len(),
            bytes: self.files.iter().map(|entry| entry.size).sum(),
        }
    }
}

impl Workspace {
    /// Create a portable, unencrypted ZIP snapshot of the workspace.
    ///
    /// `.nero` and `.git` are intentionally excluded because they are derived/cache or
    /// version-control internals. Encryption is intentionally a separate layer and will
    /// wrap this archive rather than altering the live workspace.
    pub fn create_backup(&self, destination: impl AsRef<Path>) -> Result<(PathBuf, BackupManifest)> {
        let destination = destination.as_ref();
        let destination = if destination.is_absolute() {
            destination.to_path_buf()
        } else {
            std::env::current_dir()?.join(destination)
        };

        if is_inside_workspace(&destination, &self.root()) {
            return Err(NeroError::Message(
                "backup destination must be outside the workspace to avoid backing up the backup itself".into(),
            ));
        }

        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temp = NamedTempFile::new_in(parent)?;
        let output = temp.reopen()?;
        let (_output, manifest) = write_backup_archive(self, output)?;
        temp.persist(&destination).map_err(|error| NeroError::Io(error.error))?;
        Ok((destination, manifest))
    }

    pub fn inspect_backup(archive: impl AsRef<Path>) -> Result<BackupManifest> {
        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;
        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }
        Ok(manifest)
    }

    pub fn restore_backup(archive: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<BackupStats> {
        let destination = destination.as_ref();
        if destination.exists() {
            let mut entries = fs::read_dir(destination)?;
            if entries.next().is_some() {
                return Err(NeroError::Message("restore destination must be empty".into()));
            }
        } else {
            fs::create_dir_all(destination)?;
        }

        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        drop(manifest_file);
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;
        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }

        for entry in &manifest.files {
            let relative = safe_archive_path(&entry.path)?;
            let target = destination.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut source = archive
                .by_name(&entry.path)
                .map_err(|error| NeroError::Message(format!("backup entry missing: {} ({error})", entry.path)))?;
            let mut output = fs::File::create(&target)?;
            std::io::copy(&mut source, &mut output)?;
        }

        verify_extracted_manifest(&manifest, destination)?;
        Ok(manifest.stats())
    }

    pub fn verify_backup(archive: impl AsRef<Path>) -> Result<BackupStats> {
        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        drop(manifest_file);
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;

        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }

        for entry in &manifest.files {
            let mut file = archive
                .by_name(&entry.path)
                .map_err(|error| NeroError::Message(format!("backup entry missing: {} ({error})", entry.path)))?;
            let mut hasher = Sha256::new();
            let mut size = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 { break; }
                size += read as u64;
                hasher.update(&buffer[..read]);
            }
            let hash = hex_digest(hasher.finalize());
            if size != entry.size || hash != entry.sha256 {
                return Err(NeroError::Message(format!("backup verification failed for {}", entry.path)));
            }
        }

        Ok(manifest.stats())
    }
}

pub(crate) fn write_backup_archive<W: Write>(workspace: &Workspace, output: W) -> Result<(W, BackupManifest)> {
    let entries = collect_backup_entries(workspace.root())?;
    let manifest = BackupManifest {
        format: "nero-backup".into(),
        version: MANIFEST_VERSION,
        created_at_unix: unix_now(),
        files: entries.clone(),
    };

    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| NeroError::Message(format!("could not serialize backup manifest: {error}")))?;

    // Encrypted backups wrap an age stream, which implements `Write` but not `Seek`.
    // ZIP's stream mode writes data descriptors instead of seeking back to patch headers.
    let mut zip = ZipWriter::new_stream(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(MANIFEST_NAME, options)
        .map_err(|error| NeroError::Message(format!("could not create backup manifest: {error}")))?;
    zip.write_all(&manifest_json)?;

    for entry in entries {
        let absolute = workspace.root().join(&entry.path);
        zip.start_file(&entry.path, options)
            .map_err(|error| NeroError::Message(format!("could not add {} to backup: {error}", entry.path)))?;
        let mut file = fs::File::open(&absolute)?;
        std::io::copy(&mut file, &mut zip)?;
    }

    let output = zip
        .finish()
        .map_err(|error| NeroError::Message(format!("could not finalize backup archive: {error}")))?
        .into_inner();

    Ok((output, manifest))
}

fn collect_backup_entries(root: &Path) -> Result<Vec<BackupEntry>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<BackupEntry>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        let file_type = fs::symlink_metadata(&path)?.file_type();
        if file_type.is_symlink() {
            return Err(NeroError::Message(format!("symlinks are not supported in backups: {}", path.display())));
        }
        if file_type.is_dir() {
            if name == ".nero" || name == ".git" { continue; }
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() { continue; }

        let metadata = fs::symlink_metadata(&path)?;
        let hash = sha256_file(&path)?;
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        out.push(BackupEntry { path: relative, size: metadata.len(), sha256: hash });
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut result = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn is_inside_workspace(candidate: &Path, workspace: &Path) -> bool {
    let candidate = candidate.canonicalize().unwrap_or_else(|_| candidate.to_path_buf());
    candidate.starts_with(workspace)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
fn safe_archive_path(path: &str) -> Result<PathBuf> {
    let raw = Path::new(path);
    if raw.is_absolute() {
        return Err(NeroError::Message(format!("unsafe absolute path in backup: {path}")));
    }
    let mut clean = PathBuf::new();
    for component in raw.components() {
        use std::path::Component;
        match component {
            Component::Normal(value) => clean.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(NeroError::Message(format!("unsafe path in backup: {path}")));
            }
        }
    }
    Ok(clean)
}

fn verify_extracted_manifest(manifest: &BackupManifest, destination: &Path) -> Result<()> {
    for entry in &manifest.files {
        let relative = safe_archive_path(&entry.path)?;
        let path = destination.join(relative);
        let metadata = fs::metadata(&path)?;
        let digest = sha256_file(&path)?;
        if metadata.len() != entry.size || digest != entry.sha256 {
            return Err(NeroError::Message(format!("restored file failed verification: {}", entry.path)));
        }
    }
    Ok(())
}
