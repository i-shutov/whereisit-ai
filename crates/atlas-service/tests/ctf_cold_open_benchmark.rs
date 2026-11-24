//! Manual end-to-end cold CTF opening benchmark; never part of public CI.
//! By default uses generated metadata. CTF_BENCH_COPY may name a local COPY;
//! neither paths nor catalog metadata are printed. All derivatives are temporary.
use ctf_format::{write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    sync::mpsc,
    time::{Duration, Instant},
};

static BENCHMARK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(windows)]
fn report_process_memory() {
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    // SAFETY: all-zero counters are valid output storage. The API receives the
    // exact repr(C) buffer size and a non-owning current-process pseudo-handle.
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        let size = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        counters.cb = size;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) != 0 {
            eprintln!(
                "process_peak_working_set_bytes={} process_working_set_bytes={}",
                counters.peak_working_set_size, counters.working_set_size
            );
        }
    }
}

#[cfg(not(windows))]
fn report_process_memory() {}

fn source_digest(path: &std::path::Path) -> Vec<u8> {
    let mut source = std::fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = source.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    digest.finalize().to_vec()
}

fn invoke(request: Value) -> Value {
    serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
}

fn benchmark_source(temp: &std::path::Path) -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("CTF_BENCH_COPY") {
        std::path::PathBuf::from(path)
    } else {
        let count: usize = std::env::var("CTF_BENCH_ENTRIES")
            .unwrap_or_else(|_| "300000".into())
            .parse()
            .unwrap();
        let mut entries = vec![CtfEntry {
            name: "Synthetic directory 東京".into(),
            description: String::new(),
            parent: None,
            kind: EntryKind::Directory,
            size: 0,
            attributes: 16,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: vec![],
        }];
        entries.extend((0..count).map(|n| CtfEntry {
            name: format!(
                "synthetic-{:08}-東京.mkv",
                n.wrapping_mul(2654435761) % count
            ),
            description: if n % 101 == 0 {
                "Synthetic note Москва".into()
            } else {
                String::new()
            },
            parent: Some(0),
            kind: EntryKind::File,
            size: (n as u64 * 7919) % 900000000,
            attributes: 32,
            modified_dos: 0,
            created_dos: 0,
            accessed_dos: 0,
            opaque: vec![],
        }));
        let path = temp.join("synthetic.ctf");
        std::fs::write(
            &path,
            write_ctf(&CtfCatalog {
                name: "Synthetic cold-open benchmark".into(),
                opaque_trailer: vec![],
                disks: vec![CtfDisk {
                    name: "Synthetic disk".into(),
                    opaque_header: vec![],
                    entries,
                }],
            })
            .unwrap(),
        )
        .unwrap();
        path
    }
}

#[test]
#[ignore = "manual cold-open benchmark, release mode, no concurrent benchmarks"]
fn cold_open_until_first_usable_page() {
    let _guard = BENCHMARK_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = benchmark_source(temp.path());
    let bytes_before = std::fs::metadata(&source).unwrap().len();
    let digest_before = source_digest(&source);
    let cache = temp.path().join("cache");
    let request =
        json!({"op":"ctf_sync", "path":source, "cache_root":cache, "job_id":"cold-open-benchmark"});
    let (send, receive) = mpsc::channel();
    let start = Instant::now();
    let worker = std::thread::spawn(move || send.send(invoke(request)).unwrap());
    let opened = loop {
        match receive.recv_timeout(Duration::from_secs(5)) {
            Ok(response) => break response,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let progress = invoke(json!({"op":"ctf_progress", "job_id":"cold-open-benchmark"}));
                eprintln!(
                    "elapsed_ms={} phase={} entries={}",
                    start.elapsed().as_millis(),
                    progress["data"]["phase"],
                    progress["data"]["entries"]
                );
            }
            Err(error) => panic!("worker disconnected: {error}"),
        }
    };
    worker.join().unwrap();
    assert_eq!(
        opened["ok"], true,
        "cold open failed (details intentionally omitted)"
    );
    eprintln!(
        "ready_ms={} entries={} volumes={}",
        start.elapsed().as_millis(),
        opened["data"]["entries"],
        opened["data"]["volumes"]
    );
    let page = invoke(
        json!({"op":"ctf_volumes_pinned", "path":source, "cache_root":cache, "generation":opened["data"]["generation"]}),
    );
    assert_eq!(page["ok"], true, "first page failed");
    let first_volume = page["data"]["result"].as_array().unwrap().first().unwrap()["id"].clone();
    let children = invoke(
        json!({"op":"ctf_children_pinned", "path":source, "cache_root":cache,
        "generation":opened["data"]["generation"], "volume_id":first_volume, "limit":250}),
    );
    assert_eq!(children["ok"], true, "directory page failed");
    let first_page_ms = start.elapsed().as_millis();
    assert_eq!(std::fs::metadata(&source).unwrap().len(), bytes_before);
    assert!(
        source_digest(&source) == digest_before,
        "source bytes changed"
    );
    eprintln!("first_directory_page_ms={first_page_ms}");
}

#[test]
#[ignore = "manual direct CTF opening benchmark, release mode, no concurrent benchmarks"]
fn direct_ctf_until_first_directory_page_without_indexes() {
    let _guard = BENCHMARK_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = benchmark_source(temp.path());
    let digest_before = source_digest(&source);
    let start = Instant::now();
    let opened = invoke(json!({"op":"ctf_preview_open", "path":source}));
    assert_eq!(opened["ok"], true, "direct open failed (details omitted)");
    let open_ms = start.elapsed().as_millis();
    assert_eq!(opened["data"]["state"], "browsable");
    let generation = opened["data"]["generation"].clone();
    let volumes =
        invoke(json!({"op":"ctf_preview_volumes", "path":source, "generation":generation}));
    assert_eq!(volumes["ok"], true, "direct volume list failed");
    let first = volumes["data"]["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|volume| volume["entry_count"].as_u64().unwrap_or(0) > 0)
        .unwrap()["id"]
        .clone();
    let page = invoke(json!({"op":"ctf_preview_children", "path":source,
        "generation":generation, "volume_id":first, "limit":250}));
    assert_eq!(page["ok"], true, "direct directory page failed");
    assert!(!page["data"]["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
    let first_page_ms = start.elapsed().as_millis();
    eprintln!(
        "direct_open_ms={open_ms} first_directory_page_ms={first_page_ms} entries={} volumes={}",
        opened["data"]["entries"], opened["data"]["volumes"]
    );
    let closed = invoke(json!({"op":"ctf_preview_close", "path":source, "generation":generation}));
    assert_eq!(closed["ok"], true, "direct close failed");
    assert!(
        source_digest(&source) == digest_before,
        "source bytes changed"
    );
    assert!(
        !temp.path().join("cache").exists(),
        "opening must not build search indexes"
    );
}

#[test]
#[ignore = "manual full-catalog first-search and parity benchmark; release mode only"]
fn direct_ctf_first_search_and_query_latency() {
    let _guard = BENCHMARK_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = benchmark_source(temp.path());
    let digest_before = source_digest(&source);
    let executor = std::env::var("CTF_BENCH_EXECUTOR").ok();
    let samples: usize = std::env::var("CTF_BENCH_SAMPLES")
        .unwrap_or_else(|_| "5".into())
        .parse()
        .unwrap();
    assert!((1..=100).contains(&samples));
    let reference = std::env::var_os("CTF_BENCH_SQLITE")
        .map(|path| atlas_core::Catalog::open_readonly(std::path::PathBuf::from(path)).unwrap());
    let start = Instant::now();
    let opened = invoke(json!({"op":"ctf_preview_open", "path":source}));
    assert_eq!(opened["ok"], true, "direct open failed (details omitted)");
    let open_ms = start.elapsed().as_millis();
    let generation = opened["data"]["generation"].clone();
    let first = invoke(json!({"op":"ctf_preview_search", "path":source,
        "generation":generation,"query":"mkv","limit":250,"executor":executor}));
    assert_eq!(first["ok"], true, "first direct search failed");
    if let Some(executor) = &executor {
        assert_eq!(
            first["data"]["query_executor"].as_str(),
            Some(executor.as_str())
        );
    }
    eprintln!(
        "open_ms={open_ms} open_through_first_search_ms={} entries={} volumes={} executor={}",
        start.elapsed().as_millis(),
        opened["data"]["entries"],
        opened["data"]["volumes"],
        first["data"]["query_executor"]
    );
    let queries = [
        ("empty", "", false),
        ("extension", "ext:mkv", false),
        ("default_text", "mkv", false),
        (
            "boolean",
            "(ext:mkv OR ext:mp3) AND NOT name:__synthetic_absent__",
            false,
        ),
        ("unicode", "name:東京", false),
        ("absent_name", "name:__synthetic_absent_716291__", false),
        ("absent_default_text", "__synthetic_absent_716291__", false),
        ("absent_path", "path:__synthetic_absent_716291__", false),
        (
            "absent_description",
            "description:__synthetic_absent_716291__",
            false,
        ),
        ("size_count", "ext:mkv AND size:>1MiB", true),
    ];
    for (label, query, full_count) in queries {
        let expected = reference.as_ref().map(|reference| {
            if full_count {
                serde_json::to_value(reference.search(query, None, 0, 250).unwrap()).unwrap()
            } else {
                serde_json::to_value(reference.search_fast(query, None, 0, 250).unwrap()).unwrap()
            }
        });
        let mut timings = Vec::with_capacity(samples);
        for _ in 0..samples {
            let request_start = Instant::now();
            let response = invoke(
                json!({"op":"ctf_preview_search","path":source,"generation":generation,
                "query":query,"limit":250,"offset":0,"full_count":full_count,"executor":executor}),
            );
            timings.push(request_start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(response["ok"], true, "direct query failed: {label}");
            if let Some(expected) = &expected {
                assert!(
                    &response["data"]["result"] == expected,
                    "SQLite parity failed: {label} (rows intentionally omitted)"
                );
            }
        }
        timings.sort_by(f64::total_cmp);
        let nearest_rank = |quantile: f64| {
            timings[((timings.len() as f64 * quantile).ceil() as usize).saturating_sub(1)]
        };
        eprintln!(
            "class={label} samples={samples} p50_ms={:.3} p95_ms={:.3}",
            nearest_rank(0.5),
            nearest_rank(0.95)
        );
    }
    assert!(
        source_digest(&source) == digest_before,
        "source bytes changed"
    );
    assert!(!temp.path().join("cache").exists());
    report_process_memory();
    assert_eq!(
        invoke(json!({"op":"ctf_preview_close","path":source,"generation":generation}))["ok"],
        true
    );
}

#[test]
#[ignore = "manual full-catalog direct analytics benchmark and SQLite parity, release mode only"]
fn direct_ctf_analytics_latency_and_parity() {
    let _guard = BENCHMARK_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let source = benchmark_source(temp.path());
    let digest_before = source_digest(&source);
    let samples: usize = std::env::var("CTF_BENCH_SAMPLES")
        .unwrap_or_else(|_| "5".into())
        .parse()
        .unwrap();
    assert!((1..=100).contains(&samples));
    let reference = std::env::var_os("CTF_BENCH_SQLITE")
        .map(|path| atlas_core::Catalog::open_readonly(std::path::PathBuf::from(path)).unwrap());
    let opened = invoke(json!({"op":"ctf_preview_open","path":source}));
    assert_eq!(opened["ok"], true, "direct open failed (details omitted)");
    let generation = opened["data"]["generation"].clone();
    let volumes = invoke(json!({"op":"ctf_preview_volumes","path":source,"generation":generation}));
    let largest_volume = volumes["data"]["result"]
        .as_array()
        .unwrap()
        .iter()
        .max_by_key(|volume| volume["entry_count"].as_u64().unwrap_or(0))
        .map(|volume| volume["id"].clone())
        .unwrap();
    for (label, group, volume, highlight) in [
        ("global_tree", "volumes", Value::Null, ""),
        ("global_extensions", "extensions", Value::Null, "ext:mkv"),
        ("global_categories", "categories", Value::Null, ""),
        ("largest_volume_tree", "volumes", largest_volume, ""),
    ] {
        let request = json!({"op":"ctf_preview_analytics","path":source,"generation":generation,
            "group_by":group,"volume_id":volume,"highlight_query":highlight,
            "max_depth":4,"max_nodes":128});
        let expected = reference.as_ref().map(|catalog| {
            let typed = serde_json::from_value(request.clone()).unwrap();
            serde_json::to_value(catalog.analytics(&typed, || false).unwrap()).unwrap()
        });
        let mut timings = Vec::with_capacity(samples);
        for _ in 0..samples {
            let start = Instant::now();
            let actual = invoke(request.clone());
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(actual["ok"], true, "direct analytics failed: {label}");
            if let Some(expected) = &expected {
                assert!(
                    &actual["data"]["result"] == expected,
                    "SQLite analytics parity failed: {label} (values intentionally omitted)"
                );
            }
        }
        timings.sort_by(f64::total_cmp);
        let rank = |quantile: f64| {
            timings[((timings.len() as f64 * quantile).ceil() as usize).saturating_sub(1)]
        };
        eprintln!(
            "analytics_class={label} samples={samples} p50_ms={:.3} p95_ms={:.3}",
            rank(0.5),
            rank(0.95)
        );
    }
    report_process_memory();
    assert!(
        source_digest(&source) == digest_before,
        "source bytes changed"
    );
    assert_eq!(
        invoke(json!({"op":"ctf_preview_close","path":source,"generation":generation}))["ok"],
        true
    );
}
