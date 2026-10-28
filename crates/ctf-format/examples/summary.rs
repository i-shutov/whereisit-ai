use ctf_format::*;
use std::collections::BTreeMap;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().collect::<Vec<_>>();
    let bytes = std::fs::read(&a[1])?;
    let c = parse_ctf(&bytes, &Limits::default())?;
    let mut exts = BTreeMap::<String, (u64, u64)>::new();
    let mut size = 0u64;
    let mut files = 0;
    let mut folders = 0;
    for d in &c.disks {
        for e in &d.entries {
            if e.kind != EntryKind::File {
                folders += 1;
                continue;
            }
            files += 1;
            size += e.size;
            let ext = e
                .name
                .rsplit_once('.')
                .filter(|(a, _)| !a.is_empty())
                .map(|(_, b)| b.to_lowercase())
                .unwrap_or_default();
            let x = exts.entry(ext).or_default();
            x.0 += 1;
            x.1 += e.size;
        }
    }
    let mut top=exts.iter().map(|(ext,(count,bytes))|serde_json::json!({"extension":ext,"count":count,"bytes":bytes})).collect::<Vec<_>>();
    top.sort_by_key(|x| std::cmp::Reverse(x["count"].as_u64().unwrap()));
    top.truncate(20);
    let mut media = serde_json::Map::new();
    for (label, suffixes) in [
        (
            "video",
            vec![
                "avi", "mkv", "mp4", "mov", "m4v", "wmv", "mpg", "mpeg", "mts", "m2ts", "vob",
                "webm", "flv", "ts",
            ],
        ),
        (
            "audio",
            vec![
                "mp3", "flac", "wav", "m4a", "aac", "ogg", "wma", "ape", "alac", "opus", "aiff",
                "aif",
            ],
        ),
    ] {
        let (count, bytes) = suffixes
            .iter()
            .filter_map(|s| exts.get(*s))
            .fold((0u64, 0u64), |(c, b), (x, y)| (c + x, b + y));
        media.insert(label.into(),serde_json::json!({"count":count,"bytes":bytes,"classification":"filename suffix only; file contents not read"}));
    }
    let report = serde_json::json!({"input_bytes":bytes.len(),"disks":c.disks.len(),"files":files,"folders_and_archives":folders,"file_size_sum":size,"extension_types":exts.len(),"top_extensions":top,"media":media,"diagnostics":diagnostics(&c)?,"category_mapping":"not identified; opaque fields preserved","external_whereisit_validation":"blocked by host 32-bit execution incompatibility"});
    let out = serde_json::to_string_pretty(&report)?;
    std::fs::write(&a[2], &out)?;
    println!("{out}");
    Ok(())
}
