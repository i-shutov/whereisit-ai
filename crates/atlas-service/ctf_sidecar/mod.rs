//! Experimental source/cache binding, deliberately not wired into the native UI yet.
//! CTF is authoritative; this module never writes the source or stores annotations.
//! A successful read is valid for a captured byte snapshot, not for all future edits.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const MANIFEST_VERSION: u32 = 1;
const MANIFEST_LIMIT: u64 = 64 * 1024;
const MAX_SOURCE_BYTES: u64 = 2_000_000_000;

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub parser_version: String,
    pub index_format_version: u32,
    pub normalization_version: String,
    pub sort_version: String,
    pub index_kind: String,
    pub options_sha256: String,
    /// Required when this particular derived index also depends on native annotations.
    /// Native annotations themselves must remain outside the disposable cache.
    pub overlay_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub struct Snapshot {
    pub identity: SourceIdentity,
    /// A hint only: never used to decide whether the contents match an index.
    pub name_hint: String,
    bytes: Vec<u8>,
}

impl Snapshot {
    pub fn from_bytes(name_hint: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            identity: SourceIdentity {
                sha256: sha256(&bytes),
                size_bytes: bytes.len() as u64,
            },
            name_hint: name_hint.into(),
            bytes,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn bounded_bytes(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    if limit == u64::MAX {
        return Err(invalid("Unsupported byte limit"));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(invalid("Not a regular file or size limit exceeded"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Size limit exceeded while reading"));
    }
    Ok(bytes)
}

fn stream_identity(path: &Path, limit: u64) -> io::Result<SourceIdentity> {
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Source is not a regular file"));
    }
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = size.checked_add(n as u64).ok_or_else(|| invalid("Size overflow"))?;
        if size > limit {
            return Err(invalid("Source exceeds configured size limit"));
        }
        hash.update(&buffer[..n]);
    }
    Ok(SourceIdentity {
        sha256: format!("{:x}", hash.finalize()),
        size_bytes: size,
    })
}

/// Capture bytes, reopen the path and verify a second full digest. Detects ordinary
/// in-place edits and replacement between reads, including same-size/restored-mtime
/// changes. Does NOT lock an uncooperative external editor or prevent future changes.
/// The caller must parse the captured bytes before treating them as a valid CTF.
pub fn capture(path: &Path, max_bytes: u64) -> io::Result<Snapshot> {
    capture_with_hook(path, max_bytes, || {})
}

fn capture_with_hook<F: FnOnce()>(path: &Path, max_bytes: u64, between: F) -> io::Result<Snapshot> {
    if max_bytes == 0 || max_bytes > MAX_SOURCE_BYTES {
        return Err(invalid("Unsupported source size limit"));
    }
    let bytes = bounded_bytes(path, max_bytes)?;
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let snapshot = Snapshot::from_bytes(name, bytes);
    between();
    let verified = stream_identity(path, max_bytes)?;
    if verified != snapshot.identity {
        return Err(io::Error::new(io::ErrorKind::WouldBlock, "Source changed during capture"));
    }
    Ok(snapshot)
}

/// An optimistic conflict check, not an atomic compare-and-replace primitive.
/// A later writer race is still possible; this module intentionally has NO CTF save API.
pub fn verify_edit_base(path: &Path, base: &SourceIdentity, max_bytes: u64) -> io::Result<()> {
    if stream_identity(path, max_bytes)? != *base {
        return Err(io::Error::new(io::ErrorKind::WouldBlock, "External CTF changes conflict with edit base"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u32,
    pub source: SourceIdentity,
    pub source_name_hint: String,
    pub recipe: Recipe,
    pub artifact_sha256: String,
    pub artifact_size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheState {
    Ready,
    Missing,
    Incomplete,
    SourceChanged,
    RecipeChanged,
    Incompatible,
    Corrupt,
}

#[derive(Debug)]
pub struct CacheRead {
    pub state: CacheState,
    pub artifact: Option<Vec<u8>>,
}

fn miss(state: CacheState) -> CacheRead {
    CacheRead { state, artifact: None }
}

/// Name/path is intentionally excluded. Byte-identical copies can reuse derived
/// data, but this is NOT permission to merge their editable annotation identities.
pub fn cache_key(source: &SourceIdentity, recipe: &Recipe) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(MANIFEST_VERSION, source, recipe)).map_err(|_| invalid("Recipe encoding failed"))?;
    Ok(sha256(&encoded))
}

/// Fixed artifact basenames prevent manifest-controlled path traversal. Checks
/// recipe/source and artifact checksum before the caller decodes any index bytes.
/// A checksum detects corruption, not a malicious author who can rewrite both files.
pub fn inspect(dir: &Path, source: &SourceIdentity, recipe: &Recipe, max_artifact_bytes: u64) -> io::Result<CacheRead> {
    if !dir.exists() {
        return Ok(miss(CacheState::Missing));
    }
    let manifest_path = dir.join("manifest.json");
    let metadata = match fs::symlink_metadata(&manifest_path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(miss(CacheState::Incomplete)),
        Err(e) => return Err(e),
    };
    if !metadata.is_file() || metadata.len() > MANIFEST_LIMIT {
        return Ok(miss(CacheState::Corrupt));
    }
    let encoded = match bounded_bytes(&manifest_path, MANIFEST_LIMIT) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::InvalidData => return Ok(miss(CacheState::Corrupt)),
        Err(e) => return Err(e),
    };
    let manifest: Manifest = match serde_json::from_slice(&encoded) {
        Ok(v) => v,
        Err(_) => return Ok(miss(CacheState::Corrupt)),
    };
    if manifest.manifest_version != MANIFEST_VERSION {
        return Ok(miss(CacheState::Incompatible));
    }
    if manifest.source != *source {
        return Ok(miss(CacheState::SourceChanged));
    }
    if manifest.recipe != *recipe {
        return Ok(miss(CacheState::RecipeChanged));
    }
    if manifest.artifact_size_bytes > max_artifact_bytes {
        return Ok(miss(CacheState::Corrupt));
    }
    let artifact_path = dir.join("index.bin");
    match fs::symlink_metadata(&artifact_path) {
        Ok(m) if m.is_file() && m.len() == manifest.artifact_size_bytes => {},
        Ok(_) => return Ok(miss(CacheState::Corrupt)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(miss(CacheState::Incomplete)),
        Err(e) => return Err(e),
    }
    let artifact = match bounded_bytes(&artifact_path, max_artifact_bytes) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::InvalidData => return Ok(miss(CacheState::Corrupt)),
        Err(e) => return Err(e),
    };
    if artifact.len() as u64 != manifest.artifact_size_bytes || sha256(&artifact) != manifest.artifact_sha256 {
        return Ok(miss(CacheState::Corrupt));
    }
    Ok(CacheRead { state: CacheState::Ready, artifact: Some(artifact) })
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Publish an immutable generation in a user-controlled cache root. No annotations
/// belong here. Concurrent readers see either no directory or a complete generation.
/// A corrupt existing generation fails closed; quarantine/rebuild policy belongs to
/// the coordinator, not an implicit deletion here. Crash durability still needs
/// platform-specific acceptance, particularly directory durability on Windows.
pub fn publish(root: &Path, snapshot: &Snapshot, recipe: &Recipe, artifact: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let target = root.join(cache_key(&snapshot.identity, recipe)?);
    if target.exists() {
        let existing = inspect(&target, &snapshot.identity, recipe, artifact.len() as u64)?;
        if existing.state == CacheState::Ready && existing.artifact.as_deref() == Some(artifact) {
            return Ok(target);
        }
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Existing generation differs or is corrupt; explicit recovery required"));
    }
    let temporary = tempfile::Builder::new().prefix("building-").tempdir_in(root)?;
    write_synced(&temporary.path().join("index.bin"), artifact)?;
    let manifest = Manifest {
        manifest_version: MANIFEST_VERSION,
        source: snapshot.identity.clone(),
        source_name_hint: snapshot.name_hint.clone(),
        recipe: recipe.clone(),
        artifact_sha256: sha256(artifact),
        artifact_size_bytes: artifact.len() as u64,
    };
    let encoded = serde_json::to_vec_pretty(&manifest).map_err(|_| invalid("Manifest encoding failed"))?;
    if encoded.len() as u64 > MANIFEST_LIMIT {
        return Err(invalid("Manifest too large"));
    }
    write_synced(&temporary.path().join("manifest.json"), &encoded)?;
    sync_directory(temporary.path())?;
    match fs::rename(temporary.path(), &target) {
        Ok(()) => {},
        Err(e) => {
            let competing = inspect(&target, &snapshot.identity, recipe, artifact.len() as u64)?;
            if competing.state != CacheState::Ready || competing.artifact.as_deref() != Some(artifact) {
                return Err(e);
            }
        }
    }
    sync_directory(root)?;
    Ok(target)
}

#[cfg(test)]
mod capture_race_tests {
    use super::*;
    #[test]
    fn change_between_reads_is_not_a_verified_snapshot() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("catalog.ctf");
        fs::write(&p, b"old bytes").unwrap();
        let error = capture_with_hook(&p, 100, || fs::write(&p, b"new bytes").unwrap()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }
}
