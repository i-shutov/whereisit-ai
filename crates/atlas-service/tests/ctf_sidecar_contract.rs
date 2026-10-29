#[path = "../ctf_sidecar/mod.rs"]
mod sidecar;

use sidecar::{CacheState, Recipe, Snapshot, capture, inspect, publish, sha256, verify_edit_base};
use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};

fn recipe() -> Recipe {
    Recipe {
        parser_version: "ctf-format-1.0-rc1".into(),
        index_format_version: 1,
        normalization_version: "unicode-lower-v1".into(),
        sort_version: "name-then-id-v1".into(),
        index_kind: "test-metadata".into(),
        options_sha256: sha256(b"no-media-plugins"),
        overlay_sha256: None,
    }
}
fn fixture(root: &Path) -> (PathBuf, Snapshot, PathBuf) {
    let source = root.join("catalog.ctf");
    fs::write(&source, b"fixture byte snapshot A").unwrap();
    let snapshot = capture(&source, 1024).unwrap();
    let cache = publish(&root.join("cache"), &snapshot, &recipe(), b"index payload").unwrap();
    (source, snapshot, cache)
}
fn state(dir: &Path, source: &Snapshot, r: &Recipe) -> CacheState {
    inspect(dir, &source.identity, r, 1024).unwrap().state
}
#[test]
fn valid_source_and_recipe_return_verified_payload() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, cache) = fixture(d.path());
    let hit = inspect(&cache, &snapshot.identity, &recipe(), 1024).unwrap();
    assert_eq!(hit.state, CacheState::Ready);
    assert_eq!(hit.artifact.as_deref(), Some(b"index payload".as_slice()));
    assert_eq!(fs::read(source).unwrap(), snapshot.bytes());
}
#[test]
fn external_same_size_change_with_restored_mtime_invalidates_cache() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, cache) = fixture(d.path());
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    fs::write(&source, b"fixture byte snapshot B").unwrap();
    File::options().write(true).open(&source).unwrap().set_times(FileTimes::new().set_modified(modified)).unwrap();
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
    let changed = capture(&source, 1024).unwrap();
    assert_eq!(snapshot.identity.size_bytes, changed.identity.size_bytes);
    assert_ne!(snapshot.identity.sha256, changed.identity.sha256);
    assert_eq!(state(&cache, &changed, &recipe()), CacheState::SourceChanged);
}
#[test]
fn rename_reuses_content_cache_without_requiring_same_name() {
    let d = tempfile::tempdir().unwrap();
    let (source, before, cache) = fixture(d.path());
    let renamed = d.path().join("renamed.CTF");
    fs::rename(source, &renamed).unwrap();
    let after = capture(&renamed, 1024).unwrap();
    assert_ne!(before.name_hint, after.name_hint);
    assert_eq!(before.identity, after.identity);
    assert_eq!(state(&cache, &after, &recipe()), CacheState::Ready);
}
#[test]
fn identical_copies_reuse_only_derived_bytes() {
    let d = tempfile::tempdir().unwrap();
    let (source, _, cache) = fixture(d.path());
    let second = d.path().join("copy.ctf");
    fs::copy(source, &second).unwrap();
    assert_eq!(state(&cache, &capture(&second, 1024).unwrap(), &recipe()), CacheState::Ready);
}
#[test]
fn same_basename_different_directory_and_content_does_not_match() {
    let d = tempfile::tempdir().unwrap();
    let (_, _, cache) = fixture(d.path());
    fs::create_dir(d.path().join("other")).unwrap();
    let path = d.path().join("other/catalog.ctf");
    fs::write(&path, b"a different catalog").unwrap();
    assert_eq!(state(&cache, &capture(&path, 1024).unwrap(), &recipe()), CacheState::SourceChanged);
}
#[test]
fn changed_parser_normalizer_sort_options_and_overlay_invalidate() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, cache) = fixture(d.path());
    let original = recipe();
    let mut variants = Vec::new();
    let mut r = original.clone(); r.parser_version.push_str("-new"); variants.push(r);
    let mut r = original.clone(); r.index_format_version += 1; variants.push(r);
    let mut r = original.clone(); r.normalization_version.push_str("-new"); variants.push(r);
    let mut r = original.clone(); r.sort_version.push_str("-new"); variants.push(r);
    let mut r = original.clone(); r.index_kind = "other".into(); variants.push(r);
    let mut r = original.clone(); r.options_sha256 = sha256(b"different options"); variants.push(r);
    let mut r = original.clone(); r.overlay_sha256 = Some(sha256(b"new native categories")); variants.push(r);
    for r in variants {
        assert_eq!(state(&cache, &snapshot, &r), CacheState::RecipeChanged);
        assert_ne!(sidecar::cache_key(&snapshot.identity, &r).unwrap(), sidecar::cache_key(&snapshot.identity, &original).unwrap());
    }
}
#[test]
fn cache_rebuild_after_deletion_never_changes_original_or_native_annotations() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, cache) = fixture(d.path());
    let overlay = d.path().join("native-not-a-cache.sqlite");
    fs::write(&overlay, b"persistent native annotations").unwrap();
    fs::remove_dir_all(&cache).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Missing);
    let rebuilt = publish(&d.path().join("cache"), &snapshot, &recipe(), b"index payload").unwrap();
    assert_eq!(state(&rebuilt, &snapshot, &recipe()), CacheState::Ready);
    assert_eq!(fs::read(source).unwrap(), snapshot.bytes());
    assert_eq!(fs::read(overlay).unwrap(), b"persistent native annotations");
}
#[test]
fn tampered_and_truncated_payload_never_become_hits() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, cache) = fixture(d.path());
    fs::write(cache.join("index.bin"), b"other payload").unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Corrupt);
    fs::write(cache.join("index.bin"), b"x").unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Corrupt);
}
#[test]
fn incomplete_generation_is_not_published_as_ready() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, cache) = fixture(d.path());
    fs::remove_file(cache.join("index.bin")).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Incomplete);
    fs::remove_file(cache.join("manifest.json")).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Incomplete);
}
#[test]
fn unknown_manifest_version_fails_closed() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, cache) = fixture(d.path());
    let path = cache.join("manifest.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["manifest_version"] = serde_json::json!(999);
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Incompatible);
}
#[test]
fn malformed_and_oversized_manifest_are_rejected() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, cache) = fixture(d.path());
    fs::write(cache.join("manifest.json"), b"{bad json").unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Corrupt);
    fs::write(cache.join("manifest.json"), vec![b' '; 65537]).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Corrupt);
}
#[test]
fn configured_read_limits_are_enforced() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, cache) = fixture(d.path());
    assert!(capture(&source, 1).is_err());
    assert_eq!(inspect(&cache, &snapshot.identity, &recipe(), 1).unwrap().state, CacheState::Corrupt);
    assert_eq!(fs::read(source).unwrap(), snapshot.bytes());
}
#[test]
fn repeated_identical_publication_is_idempotent_but_cannot_replace_data() {
    let d = tempfile::tempdir().unwrap();
    let (_, snapshot, first) = fixture(d.path());
    let second = publish(&d.path().join("cache"), &snapshot, &recipe(), b"index payload").unwrap();
    assert_eq!(first, second);
    assert!(publish(&d.path().join("cache"), &snapshot, &recipe(), b"another index").is_err());
    assert_eq!(state(&first, &snapshot, &recipe()), CacheState::Ready);
}
#[test]
fn truncated_external_source_is_stale_not_the_old_valid_generation() {
    let d = tempfile::tempdir().unwrap();
    let (source, _, cache) = fixture(d.path());
    fs::write(&source, b"partial").unwrap();
    assert_eq!(state(&cache, &capture(&source, 1024).unwrap(), &recipe()), CacheState::SourceChanged);
}
#[test]
fn unsaved_edit_base_detects_an_external_save_without_writing_source() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, _) = fixture(d.path());
    verify_edit_base(&source, &snapshot.identity, 1024).unwrap();
    fs::write(&source, b"new original-editor save").unwrap();
    let error = verify_edit_base(&source, &snapshot.identity, 1024).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(fs::read(source).unwrap(), b"new original-editor save");
}
#[test]
fn empty_index_artifact_is_valid_but_not_a_ctf_validation_claim() {
    let d = tempfile::tempdir().unwrap();
    let s = Snapshot::from_bytes("test.ctf", b"not parsed here".to_vec());
    let cache = publish(d.path(), &s, &recipe(), &[]).unwrap();
    assert_eq!(inspect(&cache, &s.identity, &recipe(), 0).unwrap().state, CacheState::Ready);
    assert!(ctf_format::parse_ctf(s.bytes(), &ctf_format::Limits::default()).is_err());
}
#[cfg(unix)]
#[test]
fn cache_artifact_symlinks_are_not_loaded() {
    let d = tempfile::tempdir().unwrap();
    let (source, snapshot, cache) = fixture(d.path());
    fs::remove_file(cache.join("index.bin")).unwrap();
    std::os::unix::fs::symlink(source, cache.join("index.bin")).unwrap();
    assert_eq!(state(&cache, &snapshot, &recipe()), CacheState::Corrupt);
}
