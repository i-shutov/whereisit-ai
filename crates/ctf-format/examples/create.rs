use ctf_format::*;
fn entry(name: &str, parent: Option<usize>, kind: EntryKind, size: u64) -> CtfEntry {
    CtfEntry {
        name: name.into(),
        description: "Synthetic compatibility fixture created by Atlas".into(),
        parent,
        kind,
        size,
        attributes: if kind == EntryKind::Directory { 16 } else { 32 },
        modified_dos: 0x5d5c6000,
        created_dos: 0x5d5c6000,
        accessed_dos: 0x5d5c6000,
        opaque: Vec::new(),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let c = CtfCatalog {
        name: "Atlas synthetic interchange test".into(),
        disks: vec![CtfDisk {
            name: "Synthetic disk".into(),
            entries: vec![
                entry("Folder", None, EntryKind::Directory, 5_000_000_000),
                entry("zero.txt", None, EntryKind::File, 0),
                entry("small.txt", None, EntryKind::File, 21),
                entry(
                    "Большой файл 😀.bin",
                    Some(0),
                    EntryKind::File,
                    5_000_000_000,
                ),
            ],
            opaque_header: Vec::new(),
        }],
        opaque_trailer: Vec::new(),
    };
    let b = write_ctf(&c)?;
    let x = parse_ctf(&b, &Limits::default())?;
    assert_eq!(x.disks[0].entries.len(), 4);
    std::fs::write(std::env::args().nth(1).expect("output CTF path"), b)?;
    Ok(())
}
