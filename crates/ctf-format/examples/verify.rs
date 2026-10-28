use ctf_format::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().collect::<Vec<_>>();
    let bytes = std::fs::read(&a[1])?;
    let c = parse_ctf(&bytes, &Limits::default())?;
    let encoded = write_ctf(&c)?;
    let equal = encoded == bytes;
    let mut diff = 0;
    for (a, b) in bytes.iter().zip(&encoded) {
        if a != b {
            diff += 1;
        }
    }
    std::fs::write(&a[2], &encoded)?;
    drop(bytes);
    let d = parse_ctf(&encoded, &Limits::default())?;
    assert_eq!(c.disks.len(), d.disks.len());
    let mut checked = 0usize;
    for (x, y) in c.disks.iter().zip(&d.disks) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.entries.len(), y.entries.len());
        for (a, b) in x.entries.iter().zip(&y.entries) {
            assert_eq!(
                (
                    &a.name,
                    a.parent,
                    a.kind,
                    a.size,
                    a.attributes,
                    a.modified_dos,
                    a.created_dos,
                    a.accessed_dos,
                    &a.description
                ),
                (
                    &b.name,
                    b.parent,
                    b.kind,
                    b.size,
                    b.attributes,
                    b.modified_dos,
                    b.created_dos,
                    b.accessed_dos,
                    &b.description
                )
            );
            checked += 1;
        }
    }
    println!("Verified {checked} entries across {} disks; byte-identical={equal}, positional-byte-differences={diff}, output-bytes={}",d.disks.len(),encoded.len());
    Ok(())
}
