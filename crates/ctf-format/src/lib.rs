//! Bounded reader and writer for observed WhereIsIt Catalog 3.00 records.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfCatalog {
    pub name: String,
    pub disks: Vec<CtfDisk>,
    pub opaque_trailer: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfDisk {
    pub name: String,
    pub entries: Vec<CtfEntry>,
    pub opaque_header: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub parent: Option<usize>,
    pub kind: EntryKind,
    pub size: u64,
    pub attributes: u32,
    pub modified_dos: u32,
    pub created_dos: u32,
    pub accessed_dos: u32,
    pub opaque: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File,
    Directory,
    Archive,
}
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_entries: usize,
    pub max_disks: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 2_000_000_000,
            max_entries: 20_000_000,
            max_disks: 65535,
            max_depth: 255,
        }
    }
}
#[derive(Debug, thiserror::Error)]
#[error("CTF at byte {offset}: {message}")]
pub struct CtfError {
    pub offset: usize,
    pub message: String,
}

fn err(offset: usize, message: impl Into<String>) -> CtfError {
    CtfError {
        offset,
        message: message.into(),
    }
}
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8], p: usize) -> Self {
        Self { b, p }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], CtfError> {
        let end = self
            .p
            .checked_add(n)
            .ok_or_else(|| err(self.p, "offset overflow"))?;
        let s = self
            .b
            .get(self.p..end)
            .ok_or_else(|| err(self.p, "truncated record"))?;
        self.p = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, CtfError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, CtfError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, CtfError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, CtfError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
fn put16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_le_bytes())
}
fn put32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes())
}
fn put64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes())
}
fn patch32(b: &mut [u8], p: usize, v: usize) -> Result<(), CtfError> {
    let v = u32::try_from(v).map_err(|_| err(p, "32-bit format limit"))?;
    b.get_mut(p..p + 4)
        .ok_or_else(|| err(p, "invalid patch"))?
        .copy_from_slice(&v.to_le_bytes());
    Ok(())
}
/// Decode UTF-8 and the CESU-8 surrogate pairs emitted by older Delphi builds.
fn decode_name(raw: &[u8]) -> Result<String, CtfError> {
    let raw = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(raw);
    if let Ok(s) = std::str::from_utf8(raw) {
        if s.contains('\0') {
            return Err(err(0, "NUL in name"));
        }
        return Ok(s.to_owned());
    }
    let mut units = Vec::new();
    let mut p = 0;
    while p < raw.len() {
        let c = raw[p];
        let (n, mut v) = if c < 128 {
            (1, c as u32)
        } else if c & 0xe0 == 0xc0 {
            (2, (c & 31) as u32)
        } else if c & 0xf0 == 0xe0 {
            (3, (c & 15) as u32)
        } else if c & 0xf8 == 0xf0 {
            (4, (c & 7) as u32)
        } else {
            return Err(err(p, "invalid UTF-8/CESU-8 name"));
        };
        if p + n > raw.len() {
            return Err(err(p, "truncated encoded name"));
        }
        for &d in &raw[p + 1..p + n] {
            if d & 0xc0 != 0x80 {
                return Err(err(p, "invalid UTF-8 continuation"));
            }
            v = (v << 6) | (d & 63) as u32;
        }
        if (n == 2 && v < 128)
            || (n == 3 && v < 2048)
            || (n == 4 && v < 65536)
            || v > 0x10ffff
            || v == 0
        {
            return Err(err(p, "invalid code point"));
        }
        if v <= 65535 {
            units.push(v as u16)
        } else {
            let t = v - 65536;
            units.extend([0xd800 + (t >> 10) as u16, 0xdc00 + (t & 1023) as u16]);
        }
        p += n;
    }
    String::from_utf16(&units).map_err(|_| err(0, "unpaired surrogate in name"))
}
fn encode_name(s: &str) -> Result<Vec<u8>, CtfError> {
    if s.contains('\0') {
        return Err(err(0, "NUL in name"));
    }
    let mut b = Vec::new();
    if !s.is_ascii() {
        b.extend([0xef, 0xbb, 0xbf]);
    }
    for c in s.encode_utf16() {
        if c < 128 {
            b.push(c as u8)
        } else if c < 2048 {
            b.extend([0xc0 | (c >> 6) as u8, 0x80 | (c & 63) as u8]);
        } else {
            b.extend([
                0xe0 | (c >> 12) as u8,
                0x80 | ((c >> 6) & 63) as u8,
                0x80 | (c & 63) as u8,
            ]);
        }
    }
    Ok(b)
}
fn extensions(r: &mut Reader<'_>) -> Result<Vec<(u8, u32, Vec<u8>)>, CtfError> {
    let mut out = Vec::new();
    loop {
        let ty = r.u8()?;
        if ty == 0 {
            break;
        }
        let packed = r.u32()?;
        let len = if packed & 0x08000000 != 0 {
            4
        } else {
            (packed & 0xffffff) as usize
        };
        out.push((ty, packed, r.take(len)?.to_vec()));
        if out.len() > 4096 {
            return Err(err(r.p, "too many metadata fields"));
        }
    }
    Ok(out)
}
fn write_extensions(out: &mut Vec<u8>, ext: &[(u8, u32, Vec<u8>)]) {
    for (ty, p, b) in ext {
        out.push(*ty);
        put32(out, *p);
        out.extend(b)
    }
    out.push(0);
}
#[derive(Clone, Copy)]
struct Range {
    offset: usize,
    len: usize,
    count: usize,
    names: usize,
    blob: usize,
    parent: Option<usize>,
}
fn range(r: &mut Reader<'_>, blob: usize, parent: Option<usize>) -> Result<Range, CtfError> {
    Ok(Range {
        offset: r.u32()? as usize,
        len: r.u32()? as usize,
        count: r.u32()? as usize,
        names: r.u32()? as usize,
        blob,
        parent,
    })
}
fn dates(r: &mut Reader<'_>, f: u16) -> Result<(u32, u32, u32), CtfError> {
    let m = r.u32()?;
    let c = if f & 4 != 0 { r.u32()? } else { m };
    let a = if f & 8 != 0 {
        r.u32()?
    } else if f & 0x40 != 0 {
        c
    } else {
        m
    };
    Ok((m, c, a))
}
fn read_file(
    r: &mut Reader<'_>,
    parent: Option<usize>,
    blob: &[u8],
) -> Result<(CtfEntry, usize), CtfError> {
    let start = r.p;
    let f = r.u16()?;
    if f & !0x27fc != 0 {
        return Err(err(start, format!("unsupported file flags {f:#x}")));
    }
    let n = r.u8()? as usize;
    let mut name = r.take(n)?.to_vec();
    let attr = match f & 0x30 {
        0 => r.u8()? as u32,
        0x10 => 32,
        0x20 => 128,
        _ => 0,
    };
    let (mut m, mut c, mut a) = dates(r, f)?;
    let mut size = r.u32()? as u64;
    if f & 0x400 != 0 {
        size |= (r.u16()? as u64) << 32;
    }
    if f & 0x100 != 0 {
        // Archive members store CRC/date/size/packed size rather than three filesystem times.
        let _extra = r.u32()?;
        let mut q = Reader::new(r.b, start + 3 + n + usize::from(f & 0x30 == 0));
        let _crc = q.u32()?;
        m = q.u32()?;
        size = q.u32()? as u64;
        c = 0;
        a = 0;
    }
    let mut description = String::new();
    if f & 128 != 0 {
        for (ty, packed, b) in extensions(r)? {
            if ty == 1 {
                description = decode_description(packed, &b, blob).unwrap_or_default();
            }
            if ty == 12 {
                if packed & 0x08000000 != 0 {
                    return Err(err(start, "external name continuation unsupported"));
                }
                name.extend(b)
            }
        }
    }
    let bytes = name.len();
    let name = decode_name(&name).map_err(|e| err(start, e.message))?;
    Ok((
        CtfEntry {
            name,
            description,
            parent,
            kind: EntryKind::File,
            size,
            attributes: attr,
            modified_dos: m,
            created_dos: c,
            accessed_dos: a,
            opaque: r.b[start..r.p].to_vec(),
        },
        bytes,
    ))
}
/// Parse only the observed uncompressed Catalog 3.00 variant; unknown layouts fail closed.
/// Source bytes are never modified. Offsets, ranges, counts, depth and encodings are checked.
pub fn parse_ctf(bytes: &[u8], limits: &Limits) -> Result<CtfCatalog, CtfError> {
    if bytes.len() > limits.max_bytes {
        return Err(err(0, "input exceeds byte limit"));
    }
    if !bytes.starts_with(b"Catalog 3.00\0") {
        return Err(err(0, "unsupported catalog signature"));
    }
    let mut r = Reader::new(bytes, 13);
    if r.u8()? != 128 || r.u16()? != 32768 {
        return Err(err(13, "unsupported catalog flags/compression"));
    }
    let count = r.u16()? as usize;
    if count > limits.max_disks {
        return Err(err(16, "disk count exceeds limit"));
    }
    for expected in 1..=count {
        if r.u16()? as usize != expected {
            return Err(err(r.p - 2, "unsupported catalog ordering identifiers"));
        }
    }
    let n = r.u16()? as usize;
    let name = decode_name(r.take(n)?)?;
    if r.take(5)? != [6, 184, 0, 0, 4] {
        return Err(err(r.p - 5, "unsupported catalog settings"));
    }
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        offsets.push(r.u32()? as usize)
    }
    if r.u32()? != u32::MAX {
        return Err(err(r.p - 4, "missing offset sentinel"));
    }
    if r.take(9)?.iter().any(|x| *x != 0) {
        return Err(err(r.p - 9, "unsupported catalog metadata"));
    }
    if offsets.first().copied().unwrap_or(r.p) != r.p {
        return Err(err(r.p, "unsupported catalog header extension"));
    }
    let mut disks = Vec::new();
    let mut opaque_trailer = Vec::new();
    let mut total = 0usize;
    for (i, &start) in offsets.iter().enumerate() {
        let end = offsets.get(i + 1).copied().unwrap_or(bytes.len());
        if start > end || end > bytes.len() {
            return Err(err(start, "invalid disk range"));
        }
        let b = &bytes[start..end];
        if b.first() == Some(&37) {
            if i + 1 != offsets.len() || b.len() < 12 || b[1..9].iter().any(|x| *x != 0) {
                return Err(err(start, "only a final empty disk group is supported"));
            }
            let mut g = Reader::new(b, 9);
            let n = g.u16()? as usize;
            decode_name(g.take(n)?)?;
            if g.u8()? != 0 || g.p != b.len() {
                return Err(err(start, "unsupported disk group payload"));
            }
            opaque_trailer.extend_from_slice(b);
            continue;
        }
        let mut r = Reader::new(b, 0);
        if r.u16()? != 3 {
            return Err(err(start, "unsupported disk record"));
        }
        let flags = r.u16()?;
        if flags != 128 && flags != 192 {
            return Err(err(start + 2, "compressed/encrypted disk unsupported"));
        }
        if r.u32()? != 0 {
            return Err(err(start + 4, "unsupported disk header"));
        }
        if r.u32()? as usize != b.len() {
            return Err(err(start + 8, "disk length mismatch"));
        }
        let dirs = r.u32()? as usize;
        let root = range(&mut r, 0, None)?;
        let n = r.u16()? as usize;
        let disk_name = decode_name(r.take(n)?)?;
        let mut entries = Vec::new();
        let mut ranges = vec![root];
        let mut stack = Vec::<usize>::new();
        let mut rd = Reader::new(b, dirs);
        let mut first_data = if root.offset != u32::MAX as usize {
            root.offset
        } else {
            dirs
        };
        while rd.p < b.len() {
            if total >= limits.max_entries {
                return Err(err(start + rd.p, "entry limit exceeded"));
            }
            let rec_start = rd.p;
            let marker = rd.u16()?;
            if marker & 2 == 0 {
                return Err(err(start + rec_start, "unsupported folder marker"));
            }
            let f = rd.u16()?;
            if f & !0xbc != 0 {
                return Err(err(start + rd.p, "unsupported folder flags"));
            }
            let depth = rd.u8()? as usize;
            if depth == 0 || depth > limits.max_depth || depth > stack.len() + 1 {
                return Err(err(start + rd.p, "invalid folder depth"));
            }
            stack.truncate(depth - 1);
            let parent = stack.last().copied();
            let n = rd.u16()? as usize;
            let name = decode_name(rd.take(n)?)?;
            let blob = rd.u32()? as usize;
            let index = entries.len();
            let fr = range(&mut rd, blob, Some(index))?;
            let size = rd.u64()? / 10000;
            let attr = if f & 32 != 0 { rd.u8()? as u32 } else { 16 };
            let (m, c, a) = dates(&mut rd, f)?;
            let ext = if f & 128 != 0 {
                extensions(&mut rd)?
            } else {
                Vec::new()
            };
            let mut opaque = Vec::new();
            put32(&mut opaque, (rd.p - rec_start) as u32);
            opaque.extend_from_slice(&b[rec_start..rd.p]);
            if blob > 0 {
                let end = fr
                    .offset
                    .checked_add(blob)
                    .ok_or_else(|| err(start, "blob overflow"))?;
                opaque.extend_from_slice(
                    b.get(fr.offset..end)
                        .ok_or_else(|| err(start + fr.offset, "invalid metadata blob"))?,
                )
            }
            if fr.offset != u32::MAX as usize {
                first_data = first_data.min(fr.offset)
            }
            let description = ext
                .iter()
                .find(|x| x.0 == 1)
                .and_then(|x| decode_description(x.1, &x.2, &opaque[4 + rd.p - rec_start..]))
                .unwrap_or_default();
            entries.push(CtfEntry {
                name,
                description,
                parent,
                kind: if marker & 0x10 != 0 {
                    EntryKind::Archive
                } else {
                    EntryKind::Directory
                },
                size,
                attributes: attr,
                modified_dos: m,
                created_dos: c,
                accessed_dos: a,
                opaque,
            });
            stack.push(index);
            ranges.push(fr);
            total += 1;
        }
        if first_data < r.p || first_data > dirs {
            return Err(err(start + first_data, "invalid data start"));
        }
        let opaque_header = b[..first_data].to_vec();
        let mut intervals = Vec::new();
        for fr in &ranges {
            if fr.offset == u32::MAX as usize {
                if fr.count != 0 || fr.blob != 0 {
                    return Err(err(start, "missing nonempty file range"));
                }
                continue;
            }
            let end = fr
                .offset
                .checked_add(fr.len)
                .ok_or_else(|| err(start, "range overflow"))?;
            if fr.offset < first_data || end > dirs || fr.blob > fr.len {
                return Err(err(start + fr.offset, "invalid data range"));
            }
            intervals.push((fr.offset, end));
        }
        intervals.sort_unstable();
        let mut cursor = first_data;
        for (a, b) in intervals {
            if a != cursor {
                return Err(err(start + a, "overlapping or unaccounted data ranges"));
            }
            cursor = b;
        }
        if cursor != dirs {
            return Err(err(start + cursor, "unaccounted data before folder table"));
        }
        for fr in ranges {
            if fr.count == 0 {
                continue;
            }
            if total
                .checked_add(fr.count)
                .filter(|v| *v <= limits.max_entries)
                .is_none()
            {
                return Err(err(start, "entry limit exceeded"));
            }
            let end = fr
                .offset
                .checked_add(fr.len)
                .ok_or_else(|| err(start, "range overflow"))?;
            if end > dirs || fr.blob > fr.len {
                return Err(err(start + fr.offset, "invalid file range"));
            }
            let slice = b
                .get(fr.offset..end)
                .ok_or_else(|| err(start + fr.offset, "file range outside disk"))?;
            let mut rf = Reader::new(slice, fr.blob);
            let mut names = 0;
            for _ in 0..fr.count {
                let (e, n) = read_file(&mut rf, fr.parent, &slice[..fr.blob])
                    .map_err(|e| err(start + fr.offset + e.offset, e.message))?;
                names += n;
                entries.push(e);
                total += 1;
            }
            if rf.p != slice.len() || names != fr.names {
                return Err(err(
                    start + fr.offset,
                    "file count/size/name-length mismatch",
                ));
            }
        }
        disks.push(CtfDisk {
            name: disk_name,
            entries,
            opaque_header,
        });
    }
    Ok(CtfCatalog {
        name,
        disks,
        opaque_trailer,
    })
}
fn same(a: &CtfEntry, b: &CtfEntry) -> bool {
    a.name == b.name
        && a.description == b.description
        && a.kind == b.kind
        && a.size == b.size
        && a.attributes == b.attributes
        && a.modified_dos == b.modified_dos
        && a.created_dos == b.created_dos
        && a.accessed_dos == b.accessed_dos
}
fn file_encode(e: &CtfEntry, blob: &[u8]) -> Result<(Vec<u8>, usize), CtfError> {
    if !e.opaque.is_empty() {
        let mut r = Reader::new(&e.opaque, 0);
        let (old, n) = read_file(&mut r, e.parent, blob)?;
        if r.p != e.opaque.len() {
            return Err(err(0, "opaque file has trailing bytes"));
        }
        if same(&old, e) {
            return Ok((e.opaque.clone(), n));
        }
    }
    let raw = encode_name(&e.name)?;
    if raw.is_empty() {
        return Err(err(0, "empty file name"));
    }
    let mut ext = Vec::new();
    let mut archive_body = None;
    let mut keep = 0u16;
    if !e.opaque.is_empty() {
        let mut r = Reader::new(&e.opaque, 0);
        let f = r.u16()?;
        keep = f & 0x2100;
        let n = r.u8()? as usize;
        r.take(n)?;
        let body_start = r.p;
        if f & 0x30 == 0 {
            r.u8()?;
        }
        dates(&mut r, f)?;
        r.u32()?;
        if f & 0x400 != 0 {
            r.u16()?;
        }
        if f & 0x100 != 0 {
            r.u32()?;
            archive_body = Some((f, e.opaque[body_start..r.p].to_vec()));
        }
        if f & 128 != 0 {
            ext = extensions(&mut r)?
                .into_iter()
                .filter(|x| x.0 != 12)
                .collect();
        }
    }
    let original_description = ext
        .iter()
        .find(|x| x.0 == 1)
        .and_then(|x| decode_description(x.1, &x.2, blob))
        .unwrap_or_default();
    if e.description != original_description {
        ext.retain(|x| x.0 != 1);
        if !e.description.is_empty() {
            let mut d = encode_name(&e.description)?;
            d.push(0);
            if d.len() > 0xffffff {
                return Err(err(0, "description too long"));
            }
            ext.push((1, 0x04000000 | d.len() as u32, d));
        }
    }
    let short = raw.len().min(255);
    if raw.len() > 255 {
        let tail = raw[255..].to_vec();
        if tail.len() > 0xffffff {
            return Err(err(0, "name too long"));
        }
        ext.push((12, 0x04000000 | tail.len() as u32, tail));
    }
    let mut f = keep | 0x0c;
    if e.attributes == 32 {
        f |= 0x10;
    }
    if e.size > u32::MAX as u64 {
        if e.size >> 48 != 0 {
            return Err(err(0, "CTF file size exceeds 48-bit variant"));
        }
        f |= 0x400;
    }
    if !ext.is_empty() {
        f |= 128;
    }
    let mut body = Vec::new();
    if let Some((oldflags, mut oldbody)) = archive_body {
        f = (oldflags & !128) | if ext.is_empty() { 0 } else { 128 };
        let shift = usize::from(oldflags & 0x30 == 0);
        if e.size > u32::MAX as u64 {
            return Err(err(0, "large archived member edit unsupported"));
        }
        patch32(&mut oldbody, shift + 4, e.modified_dos as usize)?;
        patch32(&mut oldbody, shift + 8, e.size as usize)?;
        body = oldbody;
    } else {
        if f & 0x10 == 0 {
            body.push(u8::try_from(e.attributes).map_err(|_| err(0, "attributes exceed 8 bits"))?)
        }
        put32(&mut body, e.modified_dos);
        put32(&mut body, e.created_dos);
        put32(&mut body, e.accessed_dos);
        put32(&mut body, e.size as u32);
        if f & 0x400 != 0 {
            put16(&mut body, (e.size >> 32) as u16);
        }
    }
    let mut out = Vec::new();
    put16(&mut out, f);
    out.push(short as u8);
    out.extend_from_slice(&raw[..short]);
    out.extend(body);
    if !ext.is_empty() {
        write_extensions(&mut out, &ext)
    }
    Ok((out, raw.len()))
}
type FolderParts = (u16, Vec<(u8, u32, Vec<u8>)>, Vec<u8>);

fn folder_parts(e: &CtfEntry) -> Result<FolderParts, CtfError> {
    if e.opaque.is_empty() {
        return Ok((
            if e.kind == EntryKind::Archive {
                0x12
            } else {
                2
            },
            Vec::new(),
            Vec::new(),
        ));
    }
    let mut q = Reader::new(&e.opaque, 0);
    let len = q.u32()? as usize;
    let rec = q.take(len)?;
    let blob = e.opaque[q.p..].to_vec();
    let mut r = Reader::new(rec, 0);
    let marker = r.u16()?;
    let f = r.u16()?;
    r.u8()?;
    let n = r.u16()? as usize;
    r.take(n)?;
    let stored = r.u32()? as usize;
    if stored != blob.len() {
        return Err(err(0, "opaque folder blob mismatch"));
    }
    r.take(24)?;
    if f & 32 != 0 {
        r.u8()?;
    }
    dates(&mut r, f)?;
    let ext = if f & 128 != 0 {
        extensions(&mut r)?
    } else {
        Vec::new()
    };
    if r.p != rec.len() {
        return Err(err(r.p, "opaque folder trailing bytes"));
    }
    Ok((marker, ext, blob))
}
fn write_disk(d: &CtfDisk) -> Result<Vec<u8>, CtfError> {
    let mut child = vec![Vec::<usize>::new(); d.entries.len() + 1];
    for (i, e) in d.entries.iter().enumerate() {
        let key = if let Some(p) = e.parent {
            if p >= d.entries.len() || d.entries[p].kind == EntryKind::File || p == i {
                return Err(err(i, "invalid parent"));
            }
            p + 1
        } else {
            0
        };
        child[key].push(i)
    }
    let mut order = Vec::<(usize, u8)>::new();
    let mut stack = Vec::new();
    for &i in child[0].iter().rev() {
        if d.entries[i].kind != EntryKind::File {
            stack.push((i, 1u16))
        }
    }
    while let Some((i, depth)) = stack.pop() {
        if depth > 255 {
            return Err(err(i, "CTF folder depth exceeds 255"));
        }
        order.push((i, depth as u8));
        if order.len() > d.entries.len() {
            return Err(err(i, "parent cycle"));
        }
        for &j in child[i + 1].iter().rev() {
            if d.entries[j].kind != EntryKind::File {
                stack.push((j, depth + 1))
            }
        }
    }
    if order.len()
        != d.entries
            .iter()
            .filter(|x| x.kind != EntryKind::File)
            .count()
    {
        return Err(err(0, "disconnected/cyclic folder tree"));
    }
    let rawname = encode_name(&d.name)?;
    let namelen = u16::try_from(rawname.len()).map_err(|_| err(0, "disk name too long"))?;
    let mut out = if d.opaque_header.len() >= 34 {
        let mut r = Reader::new(&d.opaque_header, 32);
        let old = r.u16()? as usize;
        r.take(old)?;
        let mut h = d.opaque_header[..32].to_vec();
        put16(&mut h, namelen);
        h.extend(&rawname);
        h.extend(&d.opaque_header[r.p..]);
        h
    } else {
        generic_disk_header(&rawname)?
    };
    let mut batches = Vec::<(Option<usize>, Range, u16, Vec<(u8, u32, Vec<u8>)>)>::new();
    for parent in std::iter::once(None).chain(order.iter().map(|x| Some(x.0))) {
        let (marker, mut ext, blob) = if let Some(i) = parent {
            folder_parts(&d.entries[i])?
        } else {
            (0, Vec::new(), Vec::new())
        };
        if let Some(i) = parent {
            let old = ext
                .iter()
                .find(|x| x.0 == 1)
                .and_then(|x| decode_description(x.1, &x.2, &blob))
                .unwrap_or_default();
            if old != d.entries[i].description {
                ext.retain(|x| x.0 != 1);
                if !d.entries[i].description.is_empty() {
                    let mut text = encode_name(&d.entries[i].description)?;
                    text.push(0);
                    if text.len() > 0xffffff {
                        return Err(err(i, "description too long"));
                    }
                    ext.push((1, 0x04000000 | text.len() as u32, text));
                }
            }
        }
        let start = out.len();
        out.extend(&blob);
        let mut count = 0;
        let mut names = 0;
        for &i in &child[parent.map(|i| i + 1).unwrap_or(0)] {
            if d.entries[i].kind == EntryKind::File {
                let (b, n) = file_encode(&d.entries[i], &blob)?;
                out.extend(b);
                count += 1;
                names += n;
            }
        }
        let len = out.len() - start;
        let r = Range {
            offset: if len == 0 { u32::MAX as usize } else { start },
            len: if len == 0 { u32::MAX as usize } else { len },
            count,
            names,
            blob: blob.len(),
            parent,
        };
        batches.push((parent, r, marker, ext));
    }
    let dir_offset = out.len();
    let root = batches[0].1;
    patch32(&mut out, 16, root.offset)?;
    patch32(&mut out, 20, root.len)?;
    patch32(&mut out, 24, root.count)?;
    patch32(&mut out, 28, root.names)?;
    for ((i, depth), (_, fr, marker, ext)) in order.iter().zip(batches.iter().skip(1)) {
        out.extend(folder_encode(&d.entries[*i], *depth, *fr, *marker, ext)?);
    }

    let len = out.len();
    patch32(&mut out, 8, len)?;
    patch32(&mut out, 12, dir_offset)?;
    // These are display counters. The actual trees/ranges are rebuilt above.
    let count_base = 34 + rawname.len() + 16;
    let archived = d
        .entries
        .iter()
        .filter(|x| {
            x.kind == EntryKind::File
                && x.opaque.len() >= 2
                && u16::from_le_bytes([x.opaque[0], x.opaque[1]]) & 256 != 0
        })
        .count();
    let files = d
        .entries
        .iter()
        .filter(|x| x.kind != EntryKind::Directory)
        .count()
        - archived;
    let folders = d
        .entries
        .iter()
        .filter(|x| x.kind == EntryKind::Directory)
        .count();
    if count_base + 10 <= out.len() {
        patch32(&mut out, count_base, files)?;
        patch32(&mut out, count_base + 4, archived)?;
        out[count_base + 8..count_base + 10].copy_from_slice(&(folders as u16).to_le_bytes());
    }
    Ok(out)
}
/// Rebuild catalog tables and file/folder records, preserving opaque extension blobs.
/// Compatibility is restricted to the observed variant; caller must validate with WhereIsIt.
pub fn write_ctf(c: &CtfCatalog) -> Result<Vec<u8>, CtfError> {
    let groups = usize::from(!c.opaque_trailer.is_empty());
    let count = c.disks.len() + groups;
    let count16 = u16::try_from(count).map_err(|_| err(0, "too many disks"))?;
    let name = encode_name(&c.name)?;
    let n = u16::try_from(name.len()).map_err(|_| err(0, "catalog name too long"))?;
    let mut out = b"Catalog 3.00\0".to_vec();
    out.extend([128, 0, 128]);
    put16(&mut out, count16);
    for i in 1..=count16 {
        put16(&mut out, i)
    }
    put16(&mut out, n);
    out.extend(name);
    out.extend([6, 184, 0, 0, 4]);
    let table = out.len();
    out.resize(out.len() + count * 4, 0);
    put32(&mut out, u32::MAX);
    out.extend([0; 9]);
    for (i, d) in c.disks.iter().enumerate() {
        let pos = out.len();
        patch32(&mut out, table + i * 4, pos)?;
        out.extend(write_disk(d)?)
    }
    if groups > 0 {
        let pos = out.len();
        patch32(&mut out, table + c.disks.len() * 4, pos)?;
        out.extend(&c.opaque_trailer)
    }
    Ok(out)
}

fn decode_description(packed: u32, data: &[u8], blob: &[u8]) -> Option<String> {
    if packed >> 24 & !0x0c != 0 {
        return None;
    }
    let raw = if packed & 0x08000000 != 0 {
        let offset = u32::from_le_bytes(data.try_into().ok()?) as usize;
        blob.get(offset..offset.checked_add((packed & 0xffffff) as usize)?)?
    } else {
        data
    };
    let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
    decode_name(raw).ok()
}

/// Construct a metadata-only disk header from numeric fields observed in Catalog 3.00.
/// No source catalog names, paths, serial numbers, plugin state or descriptions are embedded.
fn generic_disk_header(name: &[u8]) -> Result<Vec<u8>, CtfError> {
    let mut h = vec![0u8; 32];
    h[0] = 3;
    h[2] = 128;
    put16(
        &mut h,
        u16::try_from(name.len()).map_err(|_| err(0, "disk name too long"))?,
    );
    h.extend(name);
    let mut fixed = [0u8; 48];
    fixed[..16].copy_from_slice(&[5, 3, 0, 0, 0, 2, 0, 0, 8, 0, 0, 0, 255, 0, 0, 0]);
    fixed[46] = 4;
    h.extend(fixed);
    let label = if name.len() <= 255 { name } else { b"Atlas" };
    h.push(label.len() as u8);
    h.extend(label);
    h.push(0);
    let mut tail = [0u8; 69];
    tail[0] = 5;
    tail[1] = 64;
    h.extend(tail);
    h.push(0);
    Ok(h)
}

fn folder_encode(
    e: &CtfEntry,
    depth: u8,
    fr: Range,
    marker: u16,
    ext: &[(u8, u32, Vec<u8>)],
) -> Result<Vec<u8>, CtfError> {
    let mut f = 0x2c;
    let mut metadata = Vec::new();
    let mut raw = encode_name(&e.name)?;
    if !e.opaque.is_empty() {
        let mut q = Reader::new(&e.opaque, 0);
        let n = q.u32()? as usize;
        let rec = q.take(n)?;
        let mut r = Reader::new(rec, 2);
        let oldflags = r.u16()?;
        r.u8()?;
        let n = r.u16()? as usize;
        let oldname = r.take(n)?;
        if decode_name(oldname)? == e.name {
            raw = oldname.to_vec();
        }
        r.take(20)?;
        let meta_start = r.p;
        let oldsize = r.u64()? / 10000;
        let attr = if oldflags & 32 != 0 {
            r.u8()? as u32
        } else {
            16
        };
        let (m, c, a) = dates(&mut r, oldflags)?;
        if oldsize == e.size
            && attr == e.attributes
            && m == e.modified_dos
            && c == e.created_dos
            && a == e.accessed_dos
        {
            f = oldflags & !128;
            metadata.extend_from_slice(&rec[meta_start..r.p]);
        }
    }
    if metadata.is_empty() {
        put64(
            &mut metadata,
            e.size
                .checked_mul(10000)
                .ok_or_else(|| err(0, "folder size overflow"))?,
        );
        metadata.push(u8::try_from(e.attributes).map_err(|_| err(0, "attributes exceed 8 bits"))?);
        put32(&mut metadata, e.modified_dos);
        put32(&mut metadata, e.created_dos);
        put32(&mut metadata, e.accessed_dos);
    }
    if !ext.is_empty() {
        f |= 128;
    }
    let mut out = Vec::new();
    put16(&mut out, marker);
    put16(&mut out, f);
    out.push(depth);
    put16(
        &mut out,
        u16::try_from(raw.len()).map_err(|_| err(0, "folder name too long"))?,
    );
    out.extend(raw);
    put32(&mut out, fr.blob as u32);
    put32(&mut out, fr.offset as u32);
    put32(&mut out, fr.len as u32);
    put32(&mut out, fr.count as u32);
    put32(&mut out, fr.names as u32);
    out.extend(metadata);
    if !ext.is_empty() {
        write_extensions(&mut out, ext)
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(name: &str, parent: Option<usize>, kind: EntryKind, size: u64) -> CtfEntry {
        CtfEntry {
            name: name.into(),
            description: String::new(),
            parent,
            kind,
            size,
            attributes: if kind == EntryKind::File { 32 } else { 16 },
            modified_dos: 0x5d075e31,
            created_dos: 0x5d075e30,
            accessed_dos: 0x5d075e32,
            opaque: Vec::new(),
        }
    }
    fn sample() -> CtfCatalog {
        CtfCatalog {
            name: "Synthetic catalogue".into(),
            disks: vec![CtfDisk {
                name: "Test disk".into(),
                entries: vec![
                    item("Папка 😀", None, EntryKind::Directory, 1),
                    item("zero.txt", None, EntryKind::File, 0),
                    item(
                        &format!("{}.txt", "Я".repeat(180)),
                        Some(0),
                        EntryKind::File,
                        5_000_000_000,
                    ),
                ],
                opaque_header: Vec::new(),
            }],
            opaque_trailer: Vec::new(),
        }
    }
    #[test]
    fn diagnostics_classifies_description_gaps_without_reading_user_text() {
        assert_eq!(
            undecoded_description_reason(0x10000003, b"abc"),
            "opaque_type1_flag_0x10_semantics_unknown"
        );
        assert_eq!(
            undecoded_description_reason(0x08000003, &0_u32.to_le_bytes()),
            "external_reference_not_diagnosable_from_entry"
        );
        assert_eq!(
            undecoded_description_reason(0x00000001, &[0]),
            "explicit_empty_inline"
        );
        assert_eq!(
            undecoded_description_reason(0x00000002, &[0xff, 0]),
            "invalid_inline_utf8_or_cesu8"
        );
        assert_eq!(
            undecoded_description_reason(0x00000004, b"book"),
            "decoded_inline_but_final_description_empty"
        );
    }

    #[test]
    fn diagnostics_separate_nonstandard_type1_from_possible_text_gaps() {
        fn mock_file(packed: u32, payload: &[u8]) -> CtfEntry {
            // Synthetic Catalog 3.00 file record with one type-1 extension.
            // Not an actual user's filename, description or metadata.
            let mut opaque = vec![0x90, 0, 1, b'x'];
            opaque.extend(0_u32.to_le_bytes()); // raw DOS modification time
            opaque.extend(12_u32.to_le_bytes()); // file size
            opaque.push(1);
            opaque.extend(packed.to_le_bytes());
            opaque.extend(payload);
            opaque.push(0); // extension terminator
            CtfEntry {
                name: "synthetic.file".into(),
                description: String::new(),
                parent: None,
                kind: EntryKind::File,
                size: 12,
                attributes: 32,
                modified_dos: 0,
                created_dos: 0,
                accessed_dos: 0,
                opaque,
            }
        }
        let catalog = CtfCatalog {
            name: "generated-only".into(),
            opaque_trailer: Vec::new(),
            disks: vec![CtfDisk {
                name: "generated-disk".into(),
                opaque_header: Vec::new(),
                entries: vec![
                    mock_file(0x1c00_0125, &[0, 0, 0, 0]),
                    mock_file(0x0400_0002, &[0xff, 0]),
                ],
            }],
        };
        let found = diagnostics(&catalog).unwrap();
        assert_eq!(found.unsupported_descriptions, 2);
        assert_eq!(found.opaque_type1_flag_0x10_entries, 1);
        assert_eq!(found.possible_text_description_gap_entries, 1);
        assert_eq!(
            found.description_gap_reasons
                .get("opaque_type1_flag_0x10_semantics_unknown"),
            Some(&1)
        );
        assert_eq!(
            found.description_gap_reasons.get("invalid_inline_utf8_or_cesu8"),
            Some(&1)
        );
        assert_eq!(
            found.description_gap_flag_histogram.get("1c"),
            Some(&1)
        );
    }

    #[test]
    fn create_edit_and_export() {
        let c = sample();
        let data = write_ctf(&c).unwrap();
        let mut d = parse_ctf(&data, &Limits::default()).unwrap();
        assert_eq!(d.disks[0].entries[2].size, 5_000_000_000);
        assert_eq!(d.disks[0].entries[2].name, c.disks[0].entries[2].name);
        d.disks[0].entries[2].name = "changed.bin".into();
        d.disks[0].entries[2].size = 42;
        d.disks[0].entries[2].description = "Описание файла".into();
        let next = write_ctf(&d).unwrap();
        let e = parse_ctf(&next, &Limits::default()).unwrap();
        assert_eq!(e.disks[0].entries[2].name, "changed.bin");
        assert_eq!(e.disks[0].entries[2].size, 42);
        assert_eq!(e.disks[0].entries[2].description, "Описание файла");
    }
    #[test]
    fn new_descriptions_are_terminated_before_name_continuations() {
        let mut source = sample();
        source.disks[0].entries[0].description = "Folder description".into();
        source.disks[0].entries[2].description = "Описание длинного имени".into();
        let encoded = write_ctf(&source).unwrap();
        let decoded = parse_ctf(&encoded, &Limits::default()).unwrap();
        let file = &decoded.disks[0].entries[2];
        assert_eq!(file.name, source.disks[0].entries[2].name);
        assert_eq!(file.description, source.disks[0].entries[2].description);
        let mut r = Reader::new(&file.opaque, 0);
        let flags = r.u16().unwrap();
        let name_len = r.u8().unwrap() as usize;
        r.take(name_len).unwrap();
        if flags & 0x30 == 0 {
            r.u8().unwrap();
        }
        dates(&mut r, flags).unwrap();
        r.u32().unwrap();
        if flags & 0x400 != 0 {
            r.u16().unwrap();
        }
        let fields = extensions(&mut r).unwrap();
        assert_eq!(fields.iter().map(|f| f.0).collect::<Vec<_>>(), vec![1, 12]);
        let description = &fields[0];
        assert_eq!(description.2.last(), Some(&0));
        assert_eq!((description.1 & 0xffffff) as usize, description.2.len());
        let folder = &decoded.disks[0].entries[0];
        assert_eq!(folder.description, "Folder description");
        let (_, fields, _) = folder_parts(folder).unwrap();
        let description = fields.iter().find(|f| f.0 == 1).unwrap();
        assert_eq!(description.2.last(), Some(&0));
        assert_eq!((description.1 & 0xffffff) as usize, description.2.len());
    }

    #[test]
    fn truncated_inputs_never_panic() {
        let b = write_ctf(&sample()).unwrap();
        for n in 0..b.len() {
            assert!(
                parse_ctf(&b[..n], &Limits::default()).is_err(),
                "accepted truncation at {n}"
            );
        }
    }
    #[test]
    fn limits_and_invalid_parent() {
        let b = write_ctf(&sample()).unwrap();
        let limits = Limits {
            max_entries: 1,
            ..Limits::default()
        };
        assert!(parse_ctf(&b, &limits).is_err());
        let mut c = sample();
        c.disks[0].entries[0].parent = Some(2);
        assert!(write_ctf(&c).is_err());
    }
    #[test]
    fn corruption_is_rejected() {
        let mut b = write_ctf(&sample()).unwrap();
        b[0] = 0;
        assert!(parse_ctf(&b, &Limits::default()).is_err());
        let mut b = write_ctf(&sample()).unwrap();
        b[16] = 255;
        b[17] = 255;
        assert!(parse_ctf(&b, &Limits::default()).is_err());
    }
}

/// Reason categories describe decode failures, never original text.
fn undecoded_description_reason(packed: u32, data: &[u8]) -> &'static str {
    let flags = (packed >> 24) as u8;
    if flags & 0x10 != 0 {
        // A real CTF shows non-UTF-8 structured payloads under 0x1c.
        // Whether they are plugin metadata or encoded text is unknown.
        // Do NOT count these as confirmed missing human descriptions.
        "opaque_type1_flag_0x10_semantics_unknown"
    } else if flags & !0x0c != 0 {
        "unsupported_high_flags"
    } else if flags & 0x08 != 0 {
        // The parser may reference a larger disk/blob buffer which a CtfEntry
        // does not retain. A 4-byte pointer alone cannot prove a bad offset.
        "external_reference_not_diagnosable_from_entry"
    } else {
        let raw = data.strip_suffix(&[0]).unwrap_or(data);
        match decode_name(raw) {
            Ok(value) if value.is_empty() => "explicit_empty_inline",
            Ok(_) => "decoded_inline_but_final_description_empty",
            Err(_) => "invalid_inline_utf8_or_cesu8",
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ImportDiagnostics {
    /// Legacy count: a record has a nonzero type-1 payload but an empty decoded
    /// description. It includes intentional empty values; do not call all
    /// occurrences data loss without reviewing the classification histogram.
    pub unsupported_descriptions: usize,
    pub archived_files: usize,
    pub archive_containers: usize,
    pub descriptions: usize,
    /// Entries with an empty decoded description and opaque type-1 metadata
    /// carrying the unresolved high-byte 0x10 flag. May not contain text.
    #[serde(default)]
    pub opaque_type1_flag_0x10_entries: usize,
    /// Entries with empty decoded description and a type-1 extension whose
    /// flags do NOT carry 0x10. These may indicate a text decoding gap, but
    /// intentionally empty or multi-extension values also count here.
    #[serde(default)]
    pub possible_text_description_gap_entries: usize,
    /// Per-extension reason totals for records in unsupported_descriptions.
    #[serde(default)]
    pub description_gap_reasons: std::collections::BTreeMap<String, usize>,
    /// Upper high-byte flags only, never original encoded text or source path.
    #[serde(default)]
    pub description_gap_flag_histogram: std::collections::BTreeMap<String, usize>,
}
/// Count descriptions that remain preserved but could not be decoded for search.
pub fn diagnostics(c: &CtfCatalog) -> Result<ImportDiagnostics, CtfError> {
    let mut d = ImportDiagnostics::default();
    for disk in &c.disks {
        for e in &disk.entries {
            if !e.description.is_empty() {
                d.descriptions += 1;
            }
            if e.kind == EntryKind::Archive {
                d.archive_containers += 1;
            }
            let ext = if e.kind != EntryKind::File {
                folder_parts(e)?.1
            } else if e.opaque.is_empty() {
                Vec::new()
            } else {
                let mut r = Reader::new(&e.opaque, 0);
                let f = r.u16()?;
                if f & 256 != 0 {
                    d.archived_files += 1;
                }
                let n = r.u8()? as usize;
                r.take(n)?;
                if f & 0x30 == 0 {
                    r.u8()?;
                }
                dates(&mut r, f)?;
                r.u32()?;
                if f & 1024 != 0 {
                    r.u16()?;
                }
                if f & 256 != 0 {
                    r.u32()?;
                }
                if f & 128 != 0 {
                    extensions(&mut r)?
                } else {
                    Vec::new()
                }
            };
            if e.description.is_empty() {
                let mut gap_found = false;
                let mut has_unknown_0x10 = false;
                let mut has_other_type1 = false;
                for (kind, packed, data) in &ext {
                    if *kind != 1 || packed & 0xffffff == 0 {
                        continue;
                    }
                    gap_found = true;
                    if (packed >> 24) & 0x10 != 0 {
                        has_unknown_0x10 = true;
                    } else {
                        has_other_type1 = true;
                    }
                    *d.description_gap_reasons
                        .entry(undecoded_description_reason(*packed, data).to_owned())
                        .or_insert(0) += 1;
                    *d.description_gap_flag_histogram
                        .entry(format!("{:02x}", packed >> 24))
                        .or_insert(0) += 1;
                }
                if gap_found {
                    d.unsupported_descriptions += 1;
                }
                if has_unknown_0x10 {
                    d.opaque_type1_flag_0x10_entries += 1;
                }
                if has_other_type1 {
                    d.possible_text_description_gap_entries += 1;
                }
            }
        }
    }
    Ok(d)
}
