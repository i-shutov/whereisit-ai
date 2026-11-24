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
