//! Local-only real CTF acceptance. Inputs are never written; temporary copies and
//! derived databases are removed on exit. JSON contains aggregates, not entry names.
//! Usage: cargo run --release --locked -p atlas-service --example ctf_real_acceptance -- a.CTF b.CTF
//! CI uses --self-test only, with generated CTF fixtures (never private attachments).
#[path = "../ctf_sidecar/mod.rs"]
mod sidecar;

use atlas_core::Catalog;
use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::{json, Value};
use sidecar::{CacheState, Recipe, capture, inspect, publish, sha256};
use std::{collections::BTreeMap, fs, io, path::Path, time::Instant};

fn failure(stage: &str) -> io::Error {
    io::Error::other(stage.to_owned())
}
fn service(v: Value) -> io::Result<Value> {
    let r: Value = serde_json::from_str(&atlas_service::dispatch_json(&v.to_string())).map_err(|_| failure("invalid_service_response"))?;
    if r["ok"] != true {
        return Err(failure(&format!("service:{}", r["error"]["code"].as_str().unwrap_or("unknown"))));
    }
    Ok(r["data"].clone())
}
fn measurement<F: FnMut() -> io::Result<Vec<i64>>>(mut f: F) -> io::Result<(Vec<i64>, Value)> {
    let mut samples = Vec::new();
    let mut result = Vec::new();
    for _ in 0..7 {
        let t = Instant::now();
        result = std::hint::black_box(f()?);
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let mut sorted = samples[1..].to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok((result, json!({"first_ms":samples[0],"warm_median_ms":(sorted[2]+sorted[3])/2.0,"warm_samples_ms":samples[1..],"os_cache_flushed":false})))
}
struct Row {
    id: i64,
    name: String,
    path: String,
    description: String,
    sort_key: Vec<u8>,
    extension: String,
    size: u64,
    file: bool,
}
#[derive(Clone)]
enum Predicate { All, Extension(String), NotExtension(String), Size(u64), Name(String), Path(String), Description(String), Combined }
impl Predicate {
    fn matches(&self, r: &Row) -> bool {
        match self {
            Self::All => true,
            Self::Extension(v) => r.extension == *v,
            Self::NotExtension(v) => r.extension != *v,
            Self::Size(v) => r.size > *v,
            Self::Name(v) => r.name.contains(v),
            Self::Path(v) => r.path.contains(v),
            Self::Description(v) => r.description.contains(v),
            Self::Combined => (r.extension == "mp3" || r.extension == "pdf") && r.extension != "zip" && r.size > 1024,
        }
    }
}
fn sample_term(value: &str) -> Option<String> {
    value.split(|c: char| !c.is_alphanumeric()).find(|s| s.chars().count() >= 3).map(|s| s.chars().take(8).collect())
}
fn analyze(path: &Path) -> io::Result<Value> {
    let started = Instant::now();
    let snapshot = capture(path, 2_000_000_000)?;
    let capture_ms = started.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let parsed = match ctf_format::parse_ctf(snapshot.bytes(), &Limits::default()) {
        Ok(c) => c,
        Err(e) => return Ok(json!({"status":"unsupported_or_invalid_ctf","source":snapshot.identity,"parser_offset":e.offset,"entry_names_in_report":false,"source_written":false})),
    };
    let parse_ms = t.elapsed().as_secs_f64() * 1000.0;
    let n: usize = parsed.disks.iter().map(|d| d.entries.len()).sum();
    let opaque_bytes: usize = parsed.disks.iter().flat_map(|d| &d.entries).map(|e| e.opaque.len()).sum();
    let described = parsed.disks.iter().flat_map(|d| &d.entries).filter(|e| !e.description.is_empty()).count();
    let t = Instant::now();
    let reencoded = ctf_format::write_ctf(&parsed).map_err(|_| failure("ctf_writer_failed"))?;
    let roundtrip_ms = t.elapsed().as_secs_f64() * 1000.0;
    let unchanged_equal = reencoded == snapshot.bytes();
    let mut rows = Vec::with_capacity(n);
    for disk in &parsed.disks {
        let mut paths: Vec<String> = Vec::with_capacity(disk.entries.len());
        for e in &disk.entries {
            let full = match e.parent {
                Some(parent) => format!("{}/{}", paths.get(parent).ok_or_else(|| failure("invalid_parent_order"))?, e.name),
                None => e.name.clone(),
            };
            rows.push(Row {
                id: rows.len() as i64 + 1,
                name: e.name.to_lowercase(),
                path: full.to_lowercase(),
                description: e.description.to_lowercase(),
                sort_key: e.name.bytes().map(|b| b.to_ascii_lowercase()).collect(),
                extension: Path::new(&e.name).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase(),
                size: e.size,
                file: e.kind == EntryKind::File,
            });
            paths.push(full);
        }
    }
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_unstable_by(|&a, &b| rows[a].sort_key.cmp(&rows[b].sort_key).then(rows[a].id.cmp(&rows[b].id)));
    let temp = tempfile::tempdir()?;
    let private_source = temp.path().join("input.ctf");
    fs::write(&private_source, snapshot.bytes())?;
    let database = temp.path().join("working.sqlite");
    let t = Instant::now();
    service(json!({"op":"import_ctf","db":database,"path":private_source}))?;
    let import_ms = t.elapsed().as_secs_f64() * 1000.0;
    let catalog = Catalog::open(&database).map_err(|_| failure("database_open_failed"))?;
    if catalog.stats().map_err(|_| failure("stats_failed"))?["entries"] != json!(n) {
        return Err(failure("entry_count_mismatch"));
    }
    let mut cases = vec![("all", "".to_owned(), Predicate::All)];
    for ext in ["mp3", "flac", "m4a", "pdf", "epub", "fb2", "djvu", "zip"] {
        cases.push(("extension", format!("ext:{ext}"), Predicate::Extension(ext.into())));
    }
    cases.push(("negation", "NOT ext:mp3".into(), Predicate::NotExtension("mp3".into())));
    cases.push(("large_files", "size:>4GiB".into(), Predicate::Size(4_294_967_296)));
    cases.push(("boolean", "(ext:mp3 OR ext:pdf) AND NOT ext:zip AND size:>1KiB".into(), Predicate::Combined));
    cases.push(("no_match", "name:zzzzzz_whereisit_acceptance_no_match".into(), Predicate::Name("zzzzzz_whereisit_acceptance_no_match".into())));
    if let Some(term) = rows.iter().find_map(|r| sample_term(&r.name)) {
        cases.push(("sample_name", format!("name:{term}"), Predicate::Name(term)));
    }
    if let Some(term) = rows.iter().find_map(|r| sample_term(&r.path)) {
        cases.push(("sample_path", format!("path:{term}"), Predicate::Path(term)));
    }
    if let Some(term) = rows.iter().find_map(|r| sample_term(&r.description)) {
        cases.push(("sample_description", format!("description:{term}"), Predicate::Description(term)));
    }
    let mut reports = Vec::new();
    for (case_id, (kind, query, predicate)) in cases.iter().enumerate() {
        let expected_total = rows.iter().filter(|r| predicate.matches(r)).count() as u64;
        let (expected, ram_timing) = measurement(|| Ok(order.iter().filter_map(|&i| predicate.matches(&rows[i]).then_some(rows[i].id)).take(250).collect()))?;
        let (got, sql_timing) = measurement(|| {
            let p = catalog.search_fast(query, None, 0, 250).map_err(|_| failure("search_fast_failed"))?;
            Ok(p.items.into_iter().map(|r| r.id).collect())
        })?;
        let total = catalog.search(query, None, 0, 1).map_err(|_| failure("count_failed"))?.total;
        if expected != got || total != expected_total {
            return Err(failure(&format!("query_parity_failed_case_{case_id}")));
        }
        reports.push(json!({"case_id":case_id,"kind":kind,"query_text_omitted_for_privacy":true,"count":total,"first_page_rows":got.len(),"ram_scan_id_page":ram_timing,"native_sql_full_entry_page":sql_timing,"first_page_and_count_equal":true}));
    }
    let mut groups: BTreeMap<(String, u64), u64> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.file) {
        *groups.entry((row.name.clone(), row.size)).or_default() += 1;
    }
    let expected_members: u64 = groups.values().filter(|&&n| n > 1).sum();
    let expected_groups = groups.values().filter(|&&n| n > 1).count();
    let t = Instant::now();
    let duplicates = service(json!({"op":"duplicates","db":database,"mode":"name_size","limit":1}))?;
    let duplicate_ms = t.elapsed().as_secs_f64() * 1000.0;
    if duplicates["total"] != json!(expected_members) {
        return Err(failure("duplicate_members_mismatch"));
    }
    let csv_path = temp.path().join("selection.csv");
    let parquet_path = temp.path().join("selection.parquet");
    service(json!({"op":"export_csv","db":database,"path":csv_path}))?;
    service(json!({"op":"export_parquet","db":database,"path":parquet_path}))?;
    let mut reader = csv::Reader::from_path(&csv_path)?;
    let mut csv_rows = 0usize;
    for record in reader.records() { record?; csv_rows += 1; }
    let parquet = SerializedFileReader::new(FileForParquet::open(&parquet_path)?).map_err(|_| failure("parquet_read_failed"))?;
    if csv_rows != n || parquet.metadata().file_metadata().num_rows() != n as i64 {
        return Err(failure("export_row_count_mismatch"));
    }
    let output = temp.path().join("exported.ctf");
    service(json!({"op":"export_ctf","db":database,"path":output}))?;
    let database_roundtrip_equal = fs::read(&output)? == snapshot.bytes();
    let recipe = Recipe {
        parser_version: "ctf-format-1.0.0-rc.1".into(), index_format_version: 1,
        normalization_version: "rust-lowercase-v1".into(), sort_version: "sqlite-ascii-nocase-id-v1".into(),
        index_kind: "acceptance-summary-not-search-index".into(), options_sha256: sha256(b"no-plugins"), overlay_sha256: None,
    };
    let artifact = serde_json::to_vec(&json!({"entries":n,"volumes":parsed.disks.len()})).map_err(|_| failure("encoding_failed"))?;
    let cache = publish(&temp.path().join("cache"), &snapshot, &recipe, &artifact)?;
    if inspect(&cache, &snapshot.identity, &recipe, 1024)?.state != CacheState::Ready {
        return Err(failure("cache_miss_after_build"));
    }
    let renamed = temp.path().join("renamed.ctf");
    fs::rename(&private_source, &renamed)?;
    let moved = capture(&renamed, 2_000_000_000)?;
    if inspect(&cache, &moved.identity, &recipe, 1024)?.state != CacheState::Ready {
        return Err(failure("rename_cache_reuse_failed"));
    }
    let mut external = parsed.clone();
    external.name.push_str(" / external-save-test");
    let new_bytes = ctf_format::write_ctf(&external).map_err(|_| failure("external_save_fixture_failed"))?;
    fs::write(&renamed, new_bytes)?;
    let changed = capture(&renamed, 2_000_000_000)?;
    if inspect(&cache, &changed.identity, &recipe, 1024)?.state != CacheState::SourceChanged {
        return Err(failure("external_save_not_detected"));
    }
    if sidecar::verify_edit_base(&renamed, &snapshot.identity, 2_000_000_000).is_ok() {
        return Err(failure("dirty_edit_conflict_not_detected"));
    }
    let final_source = capture(path, 2_000_000_000)?;
    if final_source.identity != snapshot.identity {
        return Err(failure("original_source_changed_during_run"));
    }
    Ok(json!({
        "status": if unchanged_equal && database_roundtrip_equal {"passed_available_checks"} else {"roundtrip_fidelity_failed"},
        "source":snapshot.identity,"original_unchanged":true,"source_written":false,
        "capture_double_hash_ms":capture_ms,"parse_ms":parse_ms,"entries":n,"volumes":parsed.disks.len(),
        "nonempty_descriptions":described,"opaque_bytes":opaque_bytes,
        "raw_roundtrip_equal":unchanged_equal,"raw_roundtrip_encode_ms":roundtrip_ms,
        "sqlite_import_ms":import_ms,"sqlite_roundtrip_equal":database_roundtrip_equal,
        "sqlite_bytes":fs::metadata(&database)?.len(),"queries":reports,
        "duplicates":{"groups":expected_groups,"members":expected_members,"native_first_build_ms":duplicate_ms,"content_hash_verified":false},
        "export_rows_equal":true,"export_full_values_compared":false,"csv_bytes":fs::metadata(csv_path)?.len(),"parquet_bytes":fs::metadata(parquet_path)?.len(),
        "sidecars":{"rename_reuses":true,"changed_ctf_invalidates":true,"edit_base_conflict_detected":true,"artifact_is_only_test_summary":true},
        "original_WhereIsIt_open_tested":false,"ctf_legacy_categories_mapped":false,
        "limits":"First-page IDs and total counts checked for listed expressions; no full arbitrary-query proof, no UI/p95/cold-cache guarantee; no source overwrite; original application not invoked"
    }))
}
use std::fs::File as FileForParquet;
fn make_fixture(path: &Path) -> io::Result<()> {
    let mut entries = vec![CtfEntry { name:"Каталог".into(), description:String::new(), parent:None, kind:EntryKind::Directory, size:0, attributes:16, modified_dos:0, created_dos:0, accessed_dos:0, opaque:Vec::new() }];
    for (name, size, description) in [("Book.PDF", 3000, "Rare description"), ("TRACK.mp3", 7000, ""), ("track.MP3", 7000, ""), ("archive.zip", 5_000_000_000, "")] {
        entries.push(CtfEntry { name:name.into(), description:description.into(), parent:Some(0), kind:EntryKind::File, size, attributes:32, modified_dos:0, created_dos:0, accessed_dos:0, opaque:Vec::new() });
    }
    let c = CtfCatalog { name:"Synthetic acceptance fixture".into(), disks:vec![CtfDisk { name:"Synthetic disk".into(), entries, opaque_header:Vec::new() }], opaque_trailer:Vec::new() };
    fs::write(path, ctf_format::write_ctf(&c).map_err(|_| failure("fixture_failed"))?)
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: ctf_real_acceptance a.CTF b.CTF | --self-test");
        std::process::exit(2);
    }
    let mut reports = Vec::new();
    let mut success = true;
    if args == ["--self-test"] {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("fixture.ctf");
        make_fixture(&path).unwrap();
        let report = analyze(&path).unwrap();
        assert_eq!(report["status"], "passed_available_checks");
        reports.push(report);
    } else {
        for (i, arg) in args.iter().enumerate() {
            let report = match analyze(Path::new(arg)) {
                Ok(mut value) => { value["dataset_number"] = json!(i+1); value },
                Err(_) => json!({"dataset_number":i+1,"status":"failed","error_details_omitted_for_privacy":true}),
            };
            success &= report["status"] == "passed_available_checks";
            reports.push(report);
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"tool":"ctf_real_acceptance_v1","synthetic_self_test":args==["--self-test"],"reports":reports})).unwrap());
    if !success { std::process::exit(1); }
}
