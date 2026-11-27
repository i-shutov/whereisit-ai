//! Semantic parity contract for the metadata-only CTF parser.
//! The ignored large-copy check is local-only and intentionally prints aggregates only.
use ctf_format::{
    write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits, MetadataCatalog, MetadataEntry,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

#[allow(clippy::too_many_arguments)]
fn entry(
    name: &str,
    description: &str,
    parent: Option<usize>,
    kind: EntryKind,
    size: u64,
    attributes: u32,
    modified_dos: u32,
    created_dos: u32,
    accessed_dos: u32,
    opaque: &[u8],
) -> CtfEntry {
    CtfEntry {
        name: name.into(),
        description: description.into(),
        parent,
        kind,
        size,
        attributes,
        modified_dos,
        created_dos,
        accessed_dos,
        opaque: opaque.into(),
    }
}

fn synthetic_catalog() -> CtfCatalog {
    CtfCatalog {
        name: "Projection contract 東京".into(),
        disks: vec![
            CtfDisk {
                name: "Volume Ω".into(),
                entries: vec![
                    entry(
                        "資料",
                        "目录描述 — описание",
                        None,
                        EntryKind::Directory,
                        0,
                        0x10,
                        0x5d5c6000,
                        0x5d5c5000,
                        0x5d5c4000,
                        &[],
                    ),
                    entry(
                        "Видео.mkv",
                        "Кириллица и emoji 🎬",
                        Some(0),
                        EntryKind::File,
                        5_368_709_123,
                        0x20,
                        0x5d5c6001,
                        0x5d5c5001,
                        0x5d5c4001,
                        &[],
                    ),
                    entry(
                        "Архив.zip",
                        "archive description",
                        Some(0),
                        EntryKind::Archive,
                        4_294_967_296,
                        0x20,
                        0x5d5c6002,
                        0x5d5c5002,
                        0x5d5c4002,
                        &[],
                    ),
                    entry(
                        "inside.txt",
                        "nested child",
                        Some(2),
                        EntryKind::File,
                        17,
                        0x20,
                        0x5d5c6003,
                        0x5d5c5003,
                        0x5d5c4003,
                        &[],
                    ),
                ],
                opaque_header: Vec::new(),
            },
            CtfDisk {
                name: "Disk 二".into(),
                entries: vec![entry(
                    "root.dat",
                    "second volume",
                    None,
                    EntryKind::File,
                    9_000_000_000,
                    0x21,
                    0x5d5c6004,
                    0x5d5c5004,
                    0x5d5c4004,
                    &[],
                )],
                opaque_header: Vec::new(),
            },
        ],
        opaque_trailer: Vec::new(),
    }
}

fn assert_entry_equal(full: &CtfEntry, metadata: &MetadataEntry) {
    assert_eq!(full.name, metadata.name);
    assert_eq!(full.description, metadata.description);
    assert_eq!(full.parent, metadata.parent);
    assert_eq!(full.kind, metadata.kind);
    assert_eq!(full.size, metadata.size);
    assert_eq!(full.attributes, metadata.attributes);
    assert_eq!(full.modified_dos, metadata.modified_dos);
    assert_eq!(full.created_dos, metadata.created_dos);
    assert_eq!(full.accessed_dos, metadata.accessed_dos);
}

fn assert_projection_equal(full: &CtfCatalog, metadata: &MetadataCatalog) {
    assert_eq!(full.name, metadata.name);
    assert_eq!(full.disks.len(), metadata.disks.len());
    for (full_disk, metadata_disk) in full.disks.iter().zip(&metadata.disks) {
        assert_eq!(full_disk.name, metadata_disk.name);
        assert_eq!(full_disk.entries.len(), metadata_disk.entries.len());
        for (full_entry, metadata_entry) in full_disk.entries.iter().zip(&metadata_disk.entries) {
            assert_entry_equal(full_entry, metadata_entry);
        }
    }
}

fn assert_metadata_equal(left: &MetadataCatalog, right: &MetadataCatalog) {
    assert_eq!(left.name, right.name);
    assert_eq!(left.disks.len(), right.disks.len());
    for (left_disk, right_disk) in left.disks.iter().zip(&right.disks) {
        assert_eq!(left_disk.name, right_disk.name);
        assert_eq!(left_disk.entries, right_disk.entries);
    }
}

#[test]
fn metadata_parser_matches_full_semantics_and_full_roundtrip() {
    let source = synthetic_catalog();
    let original = write_ctf(&source).unwrap();
    let full = ctf_format::parse_ctf(&original, &Limits::default()).unwrap();
    let metadata = ctf_format::parse_ctf_metadata(&original, &Limits::default()).unwrap();
    assert_projection_equal(&full, &metadata);
    assert_eq!(write_ctf(&full).unwrap(), original);
    // Anchor both storage modes to the generated values as well: equality
    // between two modes alone would not catch a shared field-mapping bug.
    // CTF canonicalizes folder/file order, so resolve parents by name.
    for (expected_disk, actual_disk) in source.disks.iter().zip(&full.disks) {
        for expected in &expected_disk.entries {
            let actual = actual_disk
                .entries
                .iter()
                .find(|e| e.name == expected.name)
                .unwrap();
            assert_eq!(actual.description, expected.description);
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.size, expected.size);
            assert_eq!(actual.attributes, expected.attributes);
            assert_eq!(actual.modified_dos, expected.modified_dos);
            assert_eq!(actual.created_dos, expected.created_dos);
            assert_eq!(actual.accessed_dos, expected.accessed_dos);
            assert_eq!(
                actual.parent.map(|p| &actual_disk.entries[p].name),
                expected.parent.map(|p| &expected_disk.entries[p].name)
            );
        }
    }
    assert_eq!(full.disks.len(), 2);
    let first = &full.disks[0].entries;
    let archive_index = first.iter().position(|e| e.name == "Архив.zip").unwrap();
    assert_eq!(first[archive_index].kind, EntryKind::Archive);
    assert_eq!(
        first
            .iter()
            .find(|e| e.name == "inside.txt")
            .unwrap()
            .parent,
        Some(archive_index)
    );
    assert_eq!(
        first.iter().find(|e| e.name == "資料").unwrap().kind,
        EntryKind::Directory
    );
    assert!(first.iter().find(|e| e.name == "Видео.mkv").unwrap().size > 4_294_967_295);
    assert!(full.disks[1].entries[0].size > 4_294_967_295);
    assert_metadata_equal(&full.into_metadata(), &metadata);
}

fn source_digest(path: &Path) -> [u8; 32] {
    let mut file = File::open(path).expect("open explicitly supplied copy read-only");
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).expect("read source copy");
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    hasher.finalize().into()
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[allow(clippy::too_many_arguments)]
fn hash_common_fields(
    hasher: &mut Sha256,
    name: &str,
    description: &str,
    parent: Option<usize>,
    kind: EntryKind,
    size: u64,
    attributes: u32,
    modified_dos: u32,
    created_dos: u32,
    accessed_dos: u32,
) {
    hash_bytes(hasher, name.as_bytes());
    hash_bytes(hasher, description.as_bytes());
    match parent {
        Some(parent) => {
            hasher.update([1]);
            hasher.update((parent as u64).to_le_bytes());
        }
        None => hasher.update([0]),
    }
    hasher.update([match kind {
        EntryKind::File => 0,
        EntryKind::Directory => 1,
        EntryKind::Archive => 2,
    }]);
    hasher.update(size.to_le_bytes());
    hasher.update(attributes.to_le_bytes());
    hasher.update(modified_dos.to_le_bytes());
    hasher.update(created_dos.to_le_bytes());
    hasher.update(accessed_dos.to_le_bytes());
}

fn full_semantic_digest(catalog: &CtfCatalog) -> ([u8; 32], usize, usize) {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, catalog.name.as_bytes());
    hasher.update((catalog.disks.len() as u64).to_le_bytes());
    let mut rows = 0;
    for disk in &catalog.disks {
        hash_bytes(&mut hasher, disk.name.as_bytes());
        hasher.update((disk.entries.len() as u64).to_le_bytes());
        rows += disk.entries.len();
        for entry in &disk.entries {
            hash_common_fields(
                &mut hasher,
                &entry.name,
                &entry.description,
                entry.parent,
                entry.kind,
                entry.size,
                entry.attributes,
                entry.modified_dos,
                entry.created_dos,
                entry.accessed_dos,
            );
        }
    }
    (hasher.finalize().into(), catalog.disks.len(), rows)
}

fn metadata_semantic_digest(catalog: &MetadataCatalog) -> ([u8; 32], usize, usize) {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, catalog.name.as_bytes());
    hasher.update((catalog.disks.len() as u64).to_le_bytes());
    let mut rows = 0;
    for disk in &catalog.disks {
        hash_bytes(&mut hasher, disk.name.as_bytes());
        hasher.update((disk.entries.len() as u64).to_le_bytes());
        rows += disk.entries.len();
        for entry in &disk.entries {
            hash_common_fields(
                &mut hasher,
                &entry.name,
                &entry.description,
                entry.parent,
                entry.kind,
                entry.size,
                entry.attributes,
                entry.modified_dos,
                entry.created_dos,
                entry.accessed_dos,
            );
        }
    }
    (hasher.finalize().into(), catalog.disks.len(), rows)
}

#[test]
#[ignore = "manual local parity check; requires an explicit protected CTF copy"]
fn metadata_parser_matches_full_parser_on_explicit_copy() {
    let path = std::env::var_os("CTF_METADATA_PARITY_COPY")
        .map(std::path::PathBuf::from)
        .expect("set CTF_METADATA_PARITY_COPY to an explicit CTF COPY");
    assert!(
        path.is_file(),
        "explicit parity copy must be a regular file"
    );
    let bytes_before = std::fs::metadata(&path).unwrap().len();
    let digest_before = source_digest(&path);
    let bytes = std::fs::read(&path).expect("read explicit copy");
    assert_eq!(
        bytes.len() as u64,
        bytes_before,
        "copy changed while reading"
    );

    // Sequential parsing keeps the two implementations independent and avoids
    // retaining both parsed catalogs for the full-copy exercise.
    let full_summary = {
        let full = ctf_format::parse_ctf(&bytes, &Limits::default())
            .expect("full parser accepts explicit copy");
        assert!(
            write_ctf(&full).expect("full roundtrip succeeds") == bytes,
            "full roundtrip differs (private bytes omitted)"
        );
        full_semantic_digest(&full)
    };
    let metadata = ctf_format::parse_ctf_metadata(&bytes, &Limits::default())
        .expect("metadata parser accepts explicit copy");
    let metadata_summary = metadata_semantic_digest(&metadata);
    assert!(
        full_summary == metadata_summary,
        "common semantic fields differ (private digests omitted)"
    );

    assert_eq!(std::fs::metadata(&path).unwrap().len(), bytes_before);
    assert!(
        source_digest(&path) == digest_before,
        "source copy changed (private digests omitted)"
    );
    eprintln!(
        "metadata_parity=PASS disks={} records={} source_bytes={} source_unchanged=true",
        full_summary.1, full_summary.2, bytes_before
    );
}
