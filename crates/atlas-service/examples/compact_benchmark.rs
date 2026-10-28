//! Synthetic benchmark. It never reads the user's original catalog or media.
use atlas_core::{Catalog, NewEntry};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

fn run(db: &Path, request: Value) -> Value {
    let mut request = request;
    request["db"] = json!(db);
    let reply: Value =
        serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    reply["data"].clone()
}

fn measure(c: &Catalog, query: &str) -> (Value, Vec<i64>) {
    let mut micros = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        let page = c.search_fast(query, None, 0, 250).unwrap();
        micros.push(start.elapsed().as_micros() as u64);
        ids = page.items.into_iter().map(|entry| entry.id).collect();
    }
    let first_run_us = micros[0];
    let mut warm = micros[1..].to_vec();
    warm.sort_unstable();
    (
        json!({"query":query,"first_run_us":first_run_us,"warm_samples_us":warm,"warm_median_us":warm[2],"returned_rows":ids.len(),"cache_note":"OS caches not flushed; first-run is not claimed cold"}),
        ids,
    )
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "100000".into())
        .parse()
        .expect("row count");
    assert!((1000..=10_000_000).contains(&n));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic-v1.sqlite");
    let target = temp.path().join("synthetic-v3.sqlite");
    let connection = rusqlite::Connection::open(&source).unwrap();
    connection
        .execute_batch(include_str!("../../atlas-core/migrations/001.sql"))
        .unwrap();
    drop(connection);
    let started = Instant::now();
    let mut c = Catalog::open(&source).unwrap();
    let mut first_roots = Vec::new();
    let mut emitted = 0;
    for disk in 0..44 {
        let rows = n / 44 + u64::from(disk < n % 44);
        c.bulk_import(&format!("Synthetic disk {disk}"), "offline", |w| {
            let root = w.insert(&NewEntry {
                parent_id: None,
                name: "Медиатека".into(),
                relative_path: "Медиатека".into(),
                kind: "directory".into(),
                size: 0,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
            if disk < 6 {
                first_roots.push(root);
            }
            for local in 1..rows {
                let key = local / 2;
                let extension = if key % 3 == 0 {
                    "mkv"
                } else if key % 3 == 1 {
                    "flac"
                } else {
                    "mp3"
                };
                let name = format!("Запись-{key:07}.{extension}");
                w.insert(&NewEntry {
                    parent_id: Some(root),
                    relative_path: format!("Медиатека/{name}"),
                    name,
                    kind: "file".into(),
                    size: 5_000_000_000 + key,
                    modified_unix: Some(1_700_000_000 + key as i64),
                    description: if local % 100 == 0 {
                        "Редкое описание: музыка и фильмы".into()
                    } else {
                        String::new()
                    },
                    tags: if local % 250 == 0 {
                        "favorite".into()
                    } else {
                        String::new()
                    },
                })?;
            }
            Ok(())
        })
        .unwrap();
        emitted += rows;
        eprintln!("synthetic import: {emitted}/{n} records");
    }
    let category = c.create_category("Synthetic category", None).unwrap();
    for root in first_roots {
        c.assign_category(root, category).unwrap();
    }
    c.conn()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(c);
    let import_seconds = started.elapsed().as_secs_f64();
    let source_bytes = std::fs::metadata(&source).unwrap().len();
    let started = Instant::now();
    let conversion = run(&source, json!({"op":"compact_catalog","path":target}));
    let compact_seconds = started.elapsed().as_secs_f64();
    let target_bytes = std::fs::metadata(&target).unwrap().len();
    let a = Catalog::open(&source).unwrap();
    let b = Catalog::open(&target).unwrap();
    assert_eq!(a.stats().unwrap()["entries"], n);
    assert_eq!(a.stats().unwrap(), b.stats().unwrap());
    let mut comparisons = Vec::new();
    for q in [
        "ext:mkv AND size:>4GiB",
        "name:Запись-0000200",
        "path:Медиатека AND ext:flac",
        "description:редкое",
        "NOT tag:favorite AND ext:mp3",
        "category:\"Synthetic category\" AND ext:mkv",
        "музыка OR name:0000042",
    ] {
        let (old, old_ids) = measure(&a, q);
        let (new, new_ids) = measure(&b, q);
        assert_eq!(old_ids, new_ids, "First-page parity: {q}");
        let old_total = a.search(q, None, 0, 1).unwrap().total;
        let new_total = b.search(q, None, 0, 1).unwrap().total;
        assert_eq!(old_total, new_total, "Full-result count parity: {q}");
        comparisons
            .push(json!({"v1":old,"v3":new,"total_matches":old_total,"page_ids_equal":true}));
    }
    drop(a);
    drop(b);
    let old_duplicates = run(
        &source,
        json!({"op":"duplicates","mode":"name_size","query":"ext:mkv","limit":50}),
    );
    let new_duplicates = run(
        &target,
        json!({"op":"duplicates","mode":"name_size","query":"ext:mkv","limit":50}),
    );
    assert_eq!(old_duplicates["total"], new_duplicates["total"]);
    assert_eq!(old_duplicates["items"], new_duplicates["items"]);
    let report = json!({
        "dataset":"deterministic synthetic catalog; NOT the user's real CTF",
        "generator":"compact_benchmark v1", "records":n, "volumes":44,
        "ctf_raw_provenance_included":false,
        "v1_bytes_before_duplicate_cache":source_bytes,
        "v3_bytes_before_duplicate_cache":target_bytes,
        "reduction_percent":100.0 * (1.0 - target_bytes as f64 / source_bytes as f64),
        "v1_import_and_vacuum_seconds":import_seconds,
        "v1_to_v3_verified_copy_seconds":compact_seconds,
        "migration":conversion,"queries":comparisons,
        "duplicate_candidate_count":old_duplicates["total"],
        "duplicate_page_equal":true,
        "validation":run(&target,json!({"op":"validate_catalog"})),
        "real_data_acceptance":false,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
