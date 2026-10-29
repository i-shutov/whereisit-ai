//! Privacy-safe diagnostic of CTF description extension coverage. Reads input only.
use ctf_format::{diagnostics, parse_ctf, write_ctf, CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use serde_json::json;
use std::path::Path;

fn report(path: &Path) -> serde_json::Value {
    let result = (|| -> Result<serde_json::Value, &'static str> {
        let metadata = std::fs::metadata(path).map_err(|_| "read_failed")?;
        if !metadata.is_file() || metadata.len() > 2_000_000_000 {
            return Err("unsupported_source_size");
        }
        let bytes = std::fs::read(path).map_err(|_| "read_failed")?;
        let catalog = parse_ctf(&bytes, &Limits::default()).map_err(|_| "unsupported_ctf")?;
        let stats = diagnostics(&catalog).map_err(|_| "diagnostics_failed")?;
        let entries: usize = catalog.disks.iter().map(|disk| disk.entries.len()).sum();
        Ok(json!({
            "status":"passed",
            "disks":catalog.disks.len(),
            "entries":entries,
            "source_bytes":bytes.len(),
            "description_statistics":stats,
            "source_written":false,
            "original_names_paths_and_descriptions_exported":false,
            "notes":[
                "Legacy unsupported_descriptions includes possible intentionally empty descriptions",
                "External description pointers cannot be fully revalidated from opaque entry bytes",
                "Unknown CTF bytes are preserved; original-app edited-output compatibility not tested"
            ]
        }))
    })();
    result.unwrap_or_else(|e| json!({"status":"failed","reason":e,"source_written":false}))
}

fn self_test() {
    let dir = tempfile::tempdir().expect("temporary");
    let path = dir.path().join("generated.ctf");
    let fixture = CtfCatalog {
        name:"Synthetic only".into(), opaque_trailer:Vec::new(),
        disks:vec![CtfDisk {
            name:"volume".into(),opaque_header:Vec::new(),
            entries:vec![
                CtfEntry {
                    name:"directory".into(),description:String::new(),parent:None,
                    kind:EntryKind::Directory,size:0,attributes:16,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()
                },
                CtfEntry {
                    name:"book.pdf".into(),description:"Описание".into(),parent:Some(0),
                    kind:EntryKind::File,size:1234,attributes:32,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()
                }
            ]
        }]
    };
    std::fs::write(&path, write_ctf(&fixture).expect("synthetic encode")).expect("temporary write");
    let r = report(&path);
    assert_eq!(r["status"],"passed");
    assert_eq!(r["entries"],2);
    assert_eq!(r["description_statistics"]["unsupported_descriptions"],0);
    assert_eq!(r["description_statistics"]["descriptions"],1);
    println!("{}", json!({"self_test":"passed","generated_fixture":true}));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--self-test"] {
        self_test();return;
    }
    if args.len()!=1 {
        eprintln!("Usage: description_gap_probe PRIVATE_CTF_PATH | --self-test");
        std::process::exit(2);
    }
    let result=report(Path::new(&args[0]));
    let good = result["status"]=="passed";
    println!("{}",serde_json::to_string_pretty(&result).expect("JSON serialization"));
    if !good { std::process::exit(1); }
}
