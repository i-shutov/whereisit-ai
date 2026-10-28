use ctf_format::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let b = std::fs::read(&args[1])?;
    let mut c = parse_ctf(&b, &Limits::default())?;
    let mut files = 0;
    let mut dirs = 0;
    for d in &c.disks {
        let f = d
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::File)
            .count();
        files += f;
        dirs += d.entries.len() - f;
        println!(
            "{}\t{} files\t{} folders/archives",
            d.name,
            f,
            d.entries.len() - f
        );
    }
    println!(
        "TOTAL {} disks {} files {} folders/archives",
        c.disks.len(),
        files,
        dirs
    );
    if args.len() > 2 {
        if args.get(3).map(String::as_str) == Some("small") {
            let disk = c.disks.remove(13);
            c.disks = vec![disk];
            c.name = "Atlas compatibility test".into();
            c.opaque_trailer.clear();
        }
        let out = write_ctf(&c)?;
        let again = parse_ctf(&out, &Limits::default())?;
        assert_eq!(again.disks.len(), c.disks.len());
        for (a, b) in c.disks.iter().zip(&again.disks) {
            assert_eq!(a.entries.len(), b.entries.len());
        }
        std::fs::write(&args[2], out)?;
    }
    Ok(())
}
