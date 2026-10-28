use atlas_core::{Catalog, NewEntry};
use std::time::Instant;
fn main() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("bench.db");
    let mut db = Catalog::open(&path).unwrap();
    let t = Instant::now();
    db.bulk_import("Synthetic", "offline", |w| {
        for i in 0..100_000 {
            let name = format!(
                "Movie_{i:06}_1080p.{}",
                if i % 2 == 0 { "mkv" } else { "mp4" }
            );
            let path = format!("Media/Movies/{:03}/{}", i % 1000, name);
            w.insert(&NewEntry {
                parent_id: None,
                name,
                relative_path: path,
                kind: "file".into(),
                size: 1_000_000_000 + i as u64,
                modified_unix: None,
                description: String::new(),
                tags: String::new(),
            })?;
        }
        Ok(())
    })
    .unwrap();
    let elapsed = t.elapsed();
    db.conn()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let bytes = std::fs::metadata(&path).unwrap().len();
    println!("100k import_ms={} db_bytes={bytes}", elapsed.as_millis());
    if let Ok(mut s) = db
        .conn()
        .prepare("SELECT name, sum(pgsize) FROM dbstat GROUP BY name ORDER BY sum(pgsize) DESC")
    {
        if let Ok(rows) = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) {
            for row in rows.flatten() {
                println!("object={} bytes={}", row.0, row.1);
            }
        }
    }
    for q in [
        "name:Movie_050",
        "name:*.mkv",
        "ext:mp4 AND size:>1000040000B",
        "path:Movies/050",
    ] {
        let mut times = Vec::new();
        let mut total = 0;
        for _ in 0..20 {
            let t = Instant::now();
            let p = db.search(q, None, 0, 50).unwrap();
            total = p.total;
            times.push(t.elapsed().as_micros());
        }
        times.sort();
        println!(
            "query={q} total={total} p50_us={} p95_us={}",
            times[9], times[18]
        );
    }
}
