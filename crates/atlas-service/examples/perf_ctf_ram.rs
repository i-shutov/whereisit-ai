//! Synthetic-only in CI. Local invocation can read a private CTF, but never overwrites it.
//! Measures the existing parser, compact owned arena, ordered candidate lists, and released core.
use atlas_core::Catalog;
use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Write, path::Path, time::Instant};

const QUERIES: [&str; 13] = [
    "ext:mkv AND size:>4GiB",
    "name:Запись-0000200",
    "path:Медиатека AND ext:flac",
    "description:редкое",
    "NOT description:редкое AND ext:mp3",
    "category:featured AND ext:mkv",
    "музыка OR name:0000042",
    "name:zzzzzznomatch",
    "name:з",
    "path:album-0077 AND ext:flac",
    "name:*0000042*.mkv",
    "(ext:mkv OR ext:flac) AND NOT path:*album-0001* AND size:>1GiB",
    "size:>9000000000B",
];
fn rss() -> Value {
    let s = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |key: &str| s.lines().find(|x| x.starts_with(key)).and_then(|x| x.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok());
    json!({"rss_kib":get("VmRSS:"),"peak_rss_kib":get("VmHWM:")})
}
fn emit(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn measure<T, F: FnMut() -> T>(mut f: F) -> (T, Value) {
    let mut samples = Vec::new();
    let mut last = None;
    for _ in 0..7 {
        let t = Instant::now();
        last = Some(std::hint::black_box(f()));
        samples.push(t.elapsed().as_secs_f64()*1000.0);
    }
    let mut warm = samples[1..].to_vec();
    warm.sort_by(f64::total_cmp);
    (last.unwrap(), json!({"first_ms":samples[0],"median_ms":(warm[2]+warm[3])/2.0,"warm_samples_ms":samples[1..],"cache":"first is NOT an OS-cold measurement"}))
}
fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn write_new(path: &Path, bytes: &[u8]) {
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    f.write_all(bytes).unwrap(); f.sync_all().unwrap();
}
fn generate(n: usize, dir: &Path) {
    assert!((1000..=10_000_000).contains(&n));
    let t = Instant::now();
    let mut disks = Vec::new();
    for volume in 0..44 {
        let count = n/44 + usize::from(volume<n%44);
        let mut entries = Vec::with_capacity(count);
        let mut folder = 0;
        for local in 0..count {
            let is_dir = local==0 || local%101==1;
            let key = local/2;
            let ext = ["mkv","flac","mp3","txt"][key%4];
            let name = if local==0 { "Медиатека".to_owned() } else if is_dir { format!("album-{:04}",local/101) } else { format!("Запись-{key:07}.{ext}") };
            let size = if is_dir { 0 } else if local%10000==0 { 12_000_000_000+key as u64 } else { [5_000_000_000,60_000_000,5_000_000,1000][key%4]+key as u64 };
            entries.push(CtfEntry { name, description: if !is_dir && local%100==0 { "редкое описание: музыка и кино".into() } else { String::new() }, parent: if local==0 { None } else if is_dir { Some(0) } else { Some(folder) }, kind: if is_dir { EntryKind::Directory } else { EntryKind::File }, size, attributes: if is_dir {16} else {32}, modified_dos: 0x58210000, created_dos: 0x58210000, accessed_dos: 0x58210000, opaque: Vec::new() });
            if is_dir { folder=local; }
        }
        disks.push(CtfDisk { name:format!("Synthetic disk {volume:02}"), entries, opaque_header:Vec::new() });
    }
    let catalog = CtfCatalog { name:"PERF SYNTHETIC - no user data".into(), disks, opaque_trailer:Vec::new() };
    let generator_s=t.elapsed().as_secs_f64();
    let t=Instant::now(); let bytes=ctf_format::write_ctf(&catalog).unwrap();
    let encode_s=t.elapsed().as_secs_f64();
    let t=Instant::now(); write_new(&dir.join("source.ctf"),&bytes);
    emit(&dir.join("generate.json"), &json!({"dataset":"SYNTHETIC, not the private original","rows":n,"volumes":44,"generator_s":generator_s,"ctf_encode_s":encode_s,"ctf_write_fsync_s":t.elapsed().as_secs_f64(),"ctf_bytes":bytes.len(),"ctf_sha256":digest(&bytes),"memory":rss()}));
}
#[derive(Clone, Copy)]
struct Hot { id:u32, volume:u32, parent:u32, no:u32, nl:u32, lo:u32, ll:u32, size:u64, ext:u8, file:bool }
struct Arena { bytes:Vec<u8>, rows:Vec<Hot>, folders:HashMap<u32,(String,String)>, descriptions:HashMap<u32,String>, order:Vec<u32>, ext_order:Vec<Vec<u32>>, desc_order:Vec<u32>, large_order:Vec<u32> }
impl Arena {
    fn put(&mut self, text:&str)->(u32,u32) { let off=u32::try_from(self.bytes.len()).unwrap(); self.bytes.extend_from_slice(text.as_bytes()); (off,u32::try_from(text.len()).unwrap()) }
    fn string(&self,off:u32,len:u32)->&str { std::str::from_utf8(&self.bytes[off as usize..(off+len) as usize]).unwrap() }
    fn name(&self,r:&Hot)->&str {self.string(r.no,r.nl)}
    fn lower(&self,r:&Hot)->&str {self.string(r.lo,r.ll)}
    fn desc(&self,r:&Hot)->&str {self.descriptions.get(&r.id).map(String::as_str).unwrap_or("")}
    fn path(&self,r:&Hot)->String {
        if !r.file { return self.folders[&r.id].0.clone(); }
        if r.parent==0 { return self.name(r).to_owned(); }
        format!("{}/{}",self.folders[&r.parent].0,self.name(r))
    }
    fn path_contains(&self,r:&Hot,term:&str)->bool {
        if self.lower(r).contains(term) { return true; }
        if !r.file { return self.folders[&r.id].1.contains(term); }
        let parent=self.folders.get(&r.parent).map(|x|x.1.as_str()).unwrap_or("");
        if parent.contains(term) { return true; }
        // Only slash-containing needles can span the parent/name boundary.
        term.contains('/') && format!("{parent}/{}",self.lower(r)).contains(term)
    }
    fn matches(&self,r:&Hot,q:usize)->bool {
        match q {
            0=>r.ext==0 && r.size>4_294_967_296,
            1=>self.lower(r).contains("запись-0000200"),
            2=>r.ext==1 && self.path_contains(r,"медиатека"),
            3=>self.desc(r).contains("редкое"),
            4=>r.ext==2 && !self.desc(r).contains("редкое"),
            5=>r.ext==0 && r.volume<=6,
            6=>self.lower(r).contains("музыка") || self.path_contains(r,"музыка") || self.desc(r).contains("музыка") || self.lower(r).contains("0000042"),
            7=>self.lower(r).contains("zzzzzznomatch"),
            8=>self.lower(r).contains('з'),
            9=>r.ext==1 && self.path_contains(r,"album-0077"),
            10=>self.lower(r).contains("0000042") && self.lower(r).ends_with(".mkv"),
            11=>(r.ext==0||r.ext==1) && !self.path_contains(r,"album-0001") && r.size>1_073_741_824,
            12=>r.size>9_000_000_000,
            _=>unreachable!(),
        }
    }
    fn candidates(&self,q:usize,indexed:bool)->&[u32] {
        if !indexed {return &self.order;}
        match q {0|5|10=>&self.ext_order[0],2|9=>&self.ext_order[1],4=>&self.ext_order[2],3=>&self.desc_order,12=>&self.large_order,_=>&self.order}
    }
    fn page(&self,q:usize,indexed:bool,offset:usize)->Vec<u32> {
        self.candidates(q,indexed).iter().filter_map(|&i| { let r=&self.rows[i as usize]; self.matches(r,q).then_some(r.id) }).skip(offset).take(250).collect()
    }
    fn count(&self,q:usize,indexed:bool)->u64 {self.candidates(q,indexed).iter().filter(|&&i|self.matches(&self.rows[i as usize],q)).count() as u64}
}
fn ram(dir:&Path) {
    let t=Instant::now(); let raw=fs::read(dir.join("source.ctf")).unwrap(); let read_s=t.elapsed().as_secs_f64();
    let t=Instant::now(); let parsed=ctf_format::parse_ctf(&raw,&Limits::default()).unwrap(); let parse_s=t.elapsed().as_secs_f64();
    let parsed_memory=rss();
    let opaque_bytes:usize=parsed.disks.iter().flat_map(|d|&d.entries).map(|e|e.opaque.len()).sum();
    let t=Instant::now(); let encoded=ctf_format::write_ctf(&parsed).unwrap(); let roundtrip_encode_s=t.elapsed().as_secs_f64();
    assert_eq!(raw,encoded,"unchanged synthetic CTF must roundtrip exactly");
    let t=Instant::now(); write_new(&dir.join("roundtrip.ctf"),&encoded); let roundtrip_fsync_s=t.elapsed().as_secs_f64();
    drop(encoded);
    let t=Instant::now();
    let n:usize=parsed.disks.iter().map(|d|d.entries.len()).sum();
    let mut a=Arena{bytes:Vec::new(),rows:Vec::with_capacity(n),folders:HashMap::new(),descriptions:HashMap::new(),order:Vec::new(),ext_order:vec![Vec::new();5],desc_order:Vec::new(),large_order:Vec::new()};
    for (di,disk) in parsed.disks.iter().enumerate() {
        let base=a.rows.len() as u32;
        for (li,e) in disk.entries.iter().enumerate() {
            let id=base+li as u32+1;
            let parent=e.parent.map(|x|base+x as u32+1).unwrap_or(0);
            let (no,nl)=a.put(&e.name); let lower=e.name.to_lowercase();
            let (lo,ll)=if lower==e.name {(no,nl)} else {a.put(&lower)};
            let ext=match Path::new(&lower).extension().and_then(|x|x.to_str()).unwrap_or("") {"mkv"=>0,"flac"=>1,"mp3"=>2,"txt"=>3,_=>4};
            let file=e.kind==EntryKind::File;
            let row=Hot{id,volume:di as u32+1,parent,no,nl,lo,ll,size:e.size,ext,file};
            if !file {
                let path=if parent==0 {e.name.clone()} else {format!("{}/{}",a.folders[&parent].0,e.name)};
                a.folders.insert(id,(path.clone(),path.to_lowercase()));
            }
            if !e.description.is_empty() {a.descriptions.insert(id,e.description.to_lowercase());}
            a.rows.push(row);
        }
    }
    let arena_build_s=t.elapsed().as_secs_f64(); drop(parsed);
    let t=Instant::now();
    let mut order:Vec<u32>=(0..a.rows.len() as u32).collect();
    order.sort_unstable_by(|&x,&y|a.lower(&a.rows[x as usize]).cmp(a.lower(&a.rows[y as usize])).then_with(||x.cmp(&y)));
    for &idx in &order {
        let r=a.rows[idx as usize]; a.ext_order[r.ext as usize].push(idx);
        if a.descriptions.contains_key(&r.id) {a.desc_order.push(idx);}
        if r.size>9_000_000_000 {a.large_order.push(idx);}
    }
    a.order=order;
    let ordered_index_build_s=t.elapsed().as_secs_f64();
    let t=Instant::now();
    let mut csv=csv::WriterBuilder::new().delimiter(b'\t').from_path(dir.join("canonical.tsv")).unwrap();
    csv.write_record(["id","volume_id","parent_id","kind","name","name_norm","path","path_norm","ext","size","description","category"]).unwrap();
    for r in &a.rows {
        let path=a.path(r);
        csv.write_record([r.id.to_string(),r.volume.to_string(),r.parent.to_string(),if r.file {"file".into()} else {"directory".into()},a.name(r).into(),a.lower(r).into(),path.clone(),path.to_lowercase(),["mkv","flac","mp3","txt",""][r.ext as usize].into(),r.size.to_string(),a.desc(r).into(),u8::from(r.volume<=6).to_string()]).unwrap();
    }
    csv.flush().unwrap(); drop(csv);
    let canonical_export_s=t.elapsed().as_secs_f64();
    let mut queries=Vec::new();
    for (q,label) in QUERIES.iter().enumerate() {
        let (scan_page,scan)=measure(||a.page(q,false,0));
        let (index_page,index)=measure(||a.page(q,true,0));
        assert_eq!(scan_page,index_page);
        let (total,count)=measure(||a.count(q,true));
        assert_eq!(total,a.count(q,false));
        queries.push(json!({"id":q,"query":label,"page_ids":index_page,"total":total,"ram_scan_page":scan,"ram_ordered_candidates_page":index,"ram_count":count}));
        eprintln!("RAM Q{q}: count={total}, indexed={} ms",index["median_ms"]);
    }
    let t=Instant::now(); let mut duplicates:Vec<u32>=a.rows.iter().enumerate().filter_map(|(i,r)|r.file.then_some(i as u32)).collect();
    duplicates.sort_unstable_by(|&x,&y|a.lower(&a.rows[x as usize]).cmp(a.lower(&a.rows[y as usize])).then(a.rows[x as usize].size.cmp(&a.rows[y as usize].size)));
    let mut groups=0u64;let mut members=0u64;let mut start=0;
    while start<duplicates.len() { let first=&a.rows[duplicates[start] as usize];let mut end=start+1;
        while end<duplicates.len() {let r=&a.rows[duplicates[end] as usize];if r.size!=first.size||a.lower(r)!=a.lower(first){break;}end+=1;}
        if end-start>1 {groups+=1;members+=(end-start) as u64;}start=end;
    }
    let duplicates_s=t.elapsed().as_secs_f64();
    let (_,deep)=measure(||a.page(0,true,50_000));
    let arena_capacity_bytes=a.bytes.capacity()+a.rows.capacity()*std::mem::size_of::<Hot>()+a.order.capacity()*4+a.ext_order.iter().map(|v|v.capacity()*4).sum::<usize>()+a.desc_order.capacity()*4+a.large_order.capacity()*4;
    emit(&dir.join("ram.json"),&json!({"dataset":"synthetic CTF generator v1, no original user data","rows":a.rows.len(),"source_sha256":digest(&raw),"ctf_bytes":raw.len(),"read_s":read_s,"parse_s":parse_s,"parsed_memory":parsed_memory,"parsed_opaque_bytes":opaque_bytes,"arena_build_s":arena_build_s,"ordered_index_build_s":ordered_index_build_s,"canonical_export_s":canonical_export_s,"fixed_hot_row_bytes":std::mem::size_of::<Hot>(),"arena_major_capacity_bytes":arena_capacity_bytes,"memory_note":"capacity excludes HashMap buckets/folder/description strings and raw CTF; RSS includes allocator retention and roundtrip high-water","retained_raw_ctf_bytes":raw.len(),"memory_after_queries":rss(),"unchanged_ctf_equal":true,"roundtrip_encode_s":roundtrip_encode_s,"roundtrip_fsync_s":roundtrip_fsync_s,"duplicate_groups":groups,"duplicate_members":members,"duplicate_build_s":duplicates_s,"deep_page_50000":deep,"queries":queries}));
}
fn app(dir:&Path) {
    let db=dir.join("released-v3.sqlite");assert!(!db.exists());
    let t=Instant::now();
    let reply:Value=serde_json::from_str(&atlas_service::dispatch_json(&json!({"op":"import_ctf","db":db,"path":dir.join("source.ctf")}).to_string())).unwrap();
    assert_eq!(reply["ok"],true,"{reply}");
    let import_s=t.elapsed().as_secs_f64();
    let c=Catalog::open(&db).unwrap(); let category=c.create_category("featured",None).unwrap();
    let roots:Vec<i64>=c.conn().prepare("SELECT id FROM entries_core WHERE parent_id IS NULL AND volume_id<=6").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    for id in roots {c.assign_category(id,category).unwrap();}
    c.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let oracle:Value=serde_json::from_slice(&fs::read(dir.join("ram.json")).unwrap()).unwrap();
    let mut results=Vec::new();
    for (q,label) in QUERIES.iter().enumerate() {
        let (page,timing)=measure(||c.search_fast(label,None,0,250).unwrap());
        let ids:Vec<u32>=page.items.iter().map(|r|r.id as u32).collect();
        assert_eq!(json!(ids),oracle["queries"][q]["page_ids"],"released app first page Q{q}");
        let t=Instant::now();let total=c.search(label,None,0,1).unwrap().total;let count_ms=t.elapsed().as_secs_f64()*1000.0;
        assert_eq!(json!(total),oracle["queries"][q]["total"],"released app total Q{q}");
        results.push(json!({"id":q,"query":label,"page":timing,"total":total,"count_plus_first_row_ms":count_ms,"plan":c.explain_query(label).unwrap(),"shape":"application Entry objects with page hydration, not ID-only"}));
        eprintln!("SQLite application Q{q}: {} ms",timing["median_ms"]);
    }
    emit(&dir.join("app.json"),&json!({"import_s":import_s,"sqlite_bytes":fs::metadata(&db).unwrap().len(),"raw_ctf_provenance_included":true,"all_page_and_count_parity":true,"queries":results,"memory":rss()}));
}
fn main() {
    let args:Vec<String>=std::env::args().collect();
    assert!(args.len()>=3,"usage: perf_ctf_ram generate|ram|app DIRECTORY [COUNT]");
    let dir=Path::new(&args[2]);fs::create_dir_all(dir).unwrap();
    match args[1].as_str(){"generate"=>generate(args.get(3).map(|x|x.parse().unwrap()).unwrap_or(100_000),dir),"ram"=>ram(dir),"app"=>app(dir),_=>panic!("unknown mode")}
}
