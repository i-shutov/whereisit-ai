//! Real CTF RAM query benchmark: sorted scan versus simple field-specific indexes.
 //! Does not expose names/paths; fixed query shapes are an experiment, not production AST.
#[path="../ctf_sidecar/mod.rs"]
mod sidecar;
use ctf_format::{EntryKind,Limits};
use serde_json::{json,Value};
use std::collections::{HashMap,HashSet};
use std::path::Path;
use std::time::Instant;

struct Row {
    id:i64, name:String, path:String, description:String,
    sort:Vec<u8>, ext:String, size:u64, kind:EntryKind,
}
struct SearchIndex {
    order:Vec<usize>,
    ext:HashMap<String,Vec<usize>>,
    name_trigrams:HashMap<[u8;3],Vec<usize>>,
    described:Vec<usize>,
}
enum Condition {
    All,
    Extension(String),
    ExtensionAndLarge(String),
    Name(String),
    MissingName,
    Description(String),
    Path(String),
    NotExtension(String),
}
impl Condition {
    fn kind(&self)->&'static str {
        match self {
            Self::All=>"all",
            Self::Extension(_)=>"extension",
            Self::ExtensionAndLarge(_)=>"extension_and_large_size",
            Self::Name(_)=>"sample_name",
            Self::MissingName=>"absent_name",
            Self::Description(_)=>"sample_description",
            Self::Path(_)=>"sample_path",
            Self::NotExtension(_)=>"negative_extension",
        }
    }
    fn matches(&self,r:&Row)->bool {
        match self {
            Self::All=>true,
            Self::Extension(ext)=>r.ext==*ext,
            Self::ExtensionAndLarge(ext)=>r.ext==*ext && r.size>4_294_967_296,
            Self::Name(needle)=>r.name.contains(needle),
            Self::MissingName=>r.name.contains("zzzzzz_2026_whereisit_missing"),
            Self::Description(needle)=>r.description.contains(needle),
            Self::Path(needle)=>r.path.contains(needle),
            Self::NotExtension(ext)=>r.ext!=*ext,
        }
    }
}
fn term(text:&str)->Option<String> {
    text.split(|c:char|!c.is_alphanumeric()).find(|w|w.chars().count()>=3)
        .map(|w|w.chars().take(6).collect::<String>().to_lowercase())
}
fn memory()->Value {
    let text=std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let parse=|needle:&str| text.lines().find(|x|x.starts_with(needle))
        .and_then(|x|x.split_whitespace().nth(1)).and_then(|x|x.parse::<u64>().ok());
    json!({"rss_kib":parse("VmRSS:"),"peak_rss_kib":parse("VmHWM:"),
        "linux_proc_only":true})
}
fn build_rows(c:&ctf_format::CtfCatalog)->Result<Vec<Row>,&'static str> {
    let total:usize=c.disks.iter().map(|d|d.entries.len()).sum();
    let mut out=Vec::with_capacity(total);
    for d in &c.disks {
        let mut paths=Vec::<String>::with_capacity(d.entries.len());
        for (i,e) in d.entries.iter().enumerate() {
            let full=match e.parent {
                None=>e.name.clone(),
                Some(parent) if parent<i=>format!("{}/{}",paths[parent],e.name),
                _=>return Err("unsupported_parent_order"),
            };
            paths.push(full.clone());
            let ext=Path::new(&e.name).extension()
                .and_then(|x|x.to_str()).unwrap_or("").to_lowercase();
            let sort=e.name.as_bytes().iter().map(|b|b.to_ascii_lowercase()).collect();
            out.push(Row{id:out.len() as i64+1,name:e.name.to_lowercase(),
                path:full.to_lowercase(),description:e.description.to_lowercase(),
                sort,ext,size:e.size,kind:e.kind});
        }
    }
    Ok(out)
}
fn index(rows:&[Row])->SearchIndex {
    let mut order:Vec<usize>=(0..rows.len()).collect();
    order.sort_unstable_by(|&a,&b|rows[a].sort.cmp(&rows[b].sort).then(rows[a].id.cmp(&rows[b].id)));
    let mut ext=HashMap::<String,Vec<usize>>::new();
    let mut name_trigrams=HashMap::<[u8;3],Vec<usize>>::new();
    let mut described=Vec::new();
    for &i in &order {
        let row=&rows[i];
        ext.entry(row.ext.clone()).or_default().push(i);
        if !row.description.is_empty(){described.push(i)}
        let mut seen=HashSet::new();
        for gram in row.name.as_bytes().windows(3) {
            let key=[gram[0],gram[1],gram[2]];
            if seen.insert(key){name_trigrams.entry(key).or_default().push(i);}
        }
    }
    SearchIndex{order,ext,name_trigrams,described}
}
impl SearchIndex {
    fn candidates<'a>(&'a self,condition:&Condition)->&'a [usize] {
        match condition {
            Condition::Extension(ext)|Condition::ExtensionAndLarge(ext)=>
                self.ext.get(ext).map(Vec::as_slice).unwrap_or(&[]),
            Condition::Description(_)=>&self.described,
            Condition::Name(needle)=>{
                let mut shortest:Option<&Vec<usize>>=None;
                for gram in needle.as_bytes().windows(3) {
                    let key=[gram[0],gram[1],gram[2]];
                    let Some(postings)=self.name_trigrams.get(&key) else{return &[]};
                    if shortest.is_none_or(|x|postings.len()<x.len()) {
                        shortest=Some(postings);
                    }
                }
                shortest.map(Vec::as_slice).unwrap_or(&self.order)
            },
            Condition::MissingName=>&[],
            Condition::All|Condition::Path(_)|Condition::NotExtension(_)=>&self.order,
        }
    }
}
fn page(rows:&[Row],candidates:&[usize],condition:&Condition,offset:usize)->Vec<i64> {
    candidates.iter().filter_map(|&i|condition.matches(&rows[i]).then_some(rows[i].id))
        .skip(offset).take(250).collect()
}
fn count(rows:&[Row],candidates:&[usize],condition:&Condition)->usize {
    candidates.iter().filter(|&&i|condition.matches(&rows[i])).count()
}
fn timed<T,F:FnMut()->T>(mut callback:F)->(T,Value) {
    let mut times=Vec::with_capacity(7);
    let mut result=None;
    for _ in 0..7 {
        let start=Instant::now();
        result=Some(std::hint::black_box(callback()));
        times.push(start.elapsed().as_secs_f64()*1000.0);
    }
    let mut warm=times[1..].to_vec();warm.sort_by(f64::total_cmp);
    (result.unwrap(),json!({"first_ms":times[0],"median_warm_ms":(warm[2]+warm[3])/2.0,
        "warm_samples_ms":times[1..],"os_cache_flushed":false}))
}
fn one(path:&Path)->Value {
    let start=Instant::now();
    let snap=match sidecar::capture(path,2_000_000_000) {
        Ok(s)=>s,Err(_)=>return json!({"status":"read_failed"}),
    };
    let source=snap.identity.clone();
    let started=Instant::now();
    let c=match ctf_format::parse_ctf(snap.bytes(),&Limits::default()) {
        Ok(c)=>c,Err(e)=>return json!({"status":"parse_failed","offset":e.offset,"source":source}),
    };
    let parse_ms=started.elapsed().as_secs_f64()*1000.0;
    let started=Instant::now();
    let rows=match build_rows(&c) {
        Ok(r)=>r,Err(e)=>return json!({"status":e,"source":source}),
    };
    let rows_ms=started.elapsed().as_secs_f64()*1000.0;
    drop(c);drop(snap);
    let started=Instant::now();
    let idx=index(&rows);
    let index_ms=started.elapsed().as_secs_f64()*1000.0;
    let mem=memory();
    let top_ext=idx.ext.iter().max_by(|a,b|a.1.len().cmp(&b.1.len()))
        .map(|(k,_)|k.clone()).unwrap_or_default();
    let mut conditions=vec![
        Condition::All,Condition::Extension(top_ext.clone()),
        Condition::ExtensionAndLarge(top_ext.clone()),
        Condition::NotExtension(top_ext),
        Condition::MissingName,
    ];
    if let Some(t)=rows.iter().find_map(|r|term(&r.name)){
        conditions.push(Condition::Name(t));
    }
    if let Some(t)=rows.iter().find_map(|r|term(&r.path)){
        conditions.push(Condition::Path(t));
    }
    if let Some(t)=rows.iter().find_map(|r|term(&r.description)){
        conditions.push(Condition::Description(t));
    }
    let mut cases=Vec::new();
    let mut pass=true;
    for (case_id,condition) in conditions.iter().enumerate(){
        let (reference,page_scan)=timed(||page(&rows,&idx.order,condition,0));
        let (indexed,page_index)=timed(||page(&rows,idx.candidates(condition),condition,0));
        let (cnt_scan,count_scan)=timed(||count(&rows,&idx.order,condition));
        let (cnt_index,count_index)=timed(||count(&rows,idx.candidates(condition),condition));
        let equal=reference==indexed && cnt_scan==cnt_index;
        pass&=equal;
        cases.push(json!({"case_id":case_id,"type":condition.kind(),
            "matching_entries":cnt_scan,"first_page_entries":reference.len(),
            "first_page_equal":reference==indexed,"count_equal":cnt_scan==cnt_index,
            "scan_page":page_scan,"candidate_page":page_index,
            "scan_count":count_scan,"candidate_count":count_index,
            "query_term_omitted":true}));
    }
    let (_,deep)=timed(||page(&rows,&idx.order,&Condition::All,50_000));
    let post=sidecar::capture(path,2_000_000_000).ok()
        .map(|c|c.identity==source);
    json!({"status":if pass&&post==Some(true) {"passed"}else{"failed"},
        "source":source,"entries":rows.len(),"parse_ms":parse_ms,
        "build_rows_ms":rows_ms,"build_indexes_ms":index_ms,
        "index_types":["extension sorted postings","name trigram postings",
                       "nonempty-description list","ASCII-case-insensitive name order"],
        "memory_after_index":mem,"memory_after_queries":memory(),
        "deep_50000_all_page":deep,"cases":cases,
        "source_unchanged":post,"total_run_seconds":start.elapsed().as_secs_f64(),
        "notes":["experimental fixed query shapes; not production parser",
                 "RAM scan and indexed page/count equality checked",
                 "no exact total for unknown source format",
                 "no UI rendering, memory-mapped startup or p95 guarantee"]})
}
fn self_test(){
    let tmp=tempfile::tempdir().unwrap();
    let p=tmp.path().join("synthetic.ctf");
    let c=ctf_format::CtfCatalog{name:"Fixture".into(),opaque_trailer:Vec::new(),
        disks:vec![ctf_format::CtfDisk{name:"Disk".into(),opaque_header:Vec::new(),
            entries:vec![
                ctf_format::CtfEntry{name:"Folder".into(),description:String::new(),parent:None,
                    kind:EntryKind::Directory,size:0,attributes:16,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
                ctf_format::CtfEntry{name:"book.pdf".into(),description:"Описание".into(),parent:Some(0),
                    kind:EntryKind::File,size:100,attributes:32,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
                ctf_format::CtfEntry{name:"music.mp3".into(),description:String::new(),parent:Some(0),
                    kind:EntryKind::File,size:5_000_000_000,attributes:32,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
        ]}]};
    std::fs::write(&p,ctf_format::write_ctf(&c).unwrap()).unwrap();
    let r=one(&p);assert_eq!(r["status"],"passed");assert_eq!(r["entries"],3);
    assert_eq!(r["source_unchanged"],true);
    println!("{}",json!({"synthetic_self_test":"PASS","cases":r["cases"].as_array().unwrap().len()}));
}
fn main(){
    let args:Vec<String>=std::env::args().skip(1).collect();
    if args==["--self-test"]{self_test();return;}
    if args.is_empty(){eprintln!("Usage: ctf_real_ram_index <CTF1> [CTF2] | --self-test");std::process::exit(2);}
    let mut all=Vec::new();let mut passed=true;
    for (i,name) in args.iter().enumerate(){
        let r=one(Path::new(name));passed&=r["status"]=="passed";
        all.push(json!({"dataset_number":i+1,"result":r}));
    }
    println!("{}",serde_json::to_string_pretty(&json!({"tool":"b1_ram_index_v1",
        "private_names_exported":false,"reports":all})).unwrap());
    if !passed{std::process::exit(1)}
}
