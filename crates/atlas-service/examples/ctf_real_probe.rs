//! Read-only, aggregate-only first-pass probe for actual WhereIsIt CTF inputs.
 //! No private names, full paths, descriptions, raw bytes or serials are printed.
 //! Does not alter original CTF, does not create an index beside it.
#[path = "../ctf_sidecar/mod.rs"]
mod sidecar;
use ctf_format::{CtfCatalog, CtfDisk, CtfEntry, EntryKind, Limits};
use serde_json::{json, Value};
use sidecar::{capture, sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::Instant;

fn rss() -> Value {
    let contents = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| -> Option<u64> {
        contents.lines().find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
    };
    json!({"current_kib":field("VmRSS:"),"high_water_kib":field("VmHWM:"),
            "platform_note":"Linux /proc only; null on Windows/macOS"})
}
fn byte_bucket(n:usize)->&'static str {
    match n {0=>"empty",1..=31=>"1_31",32..=255=>"32_255",
             256..=4095=>"256_4095",_=>"4096_plus"}
}
fn size_bucket(n:u64)->&'static str {
    match n {0=>"zero",1..=1023=>"1_1023",
        1024..=1_048_575=>"1k_1m",1_048_576..=1_073_741_823=>"1m_1g",
        1_073_741_824..=4_294_967_295=>"1g_4g",_=>"4g_plus"}
}
fn incr(map:&mut BTreeMap<String,u64>,k:&str){*map.entry(k.into()).or_default()+=1;}
fn summarize(c:&CtfCatalog)->Value {
    let started=Instant::now();
    let mut kinds=BTreeMap::new();
    let mut attr=BTreeMap::new();
    let mut sizes=BTreeMap::new();
    let mut desc_lengths=BTreeMap::new();
    let mut raw_lengths=BTreeMap::new();
    let mut extensions=HashMap::<String,u64>::new();
    let mut name_bytes=0u64;
    let mut path_bytes=0u64;
    let mut desc_bytes=0u64;
    let mut raw_bytes=0u64;
    let mut non_ascii=0u64;
    let mut longest_name=0usize;
    let mut longest_path=0usize;
    let mut max_raw=0usize;
    let mut roots=0usize;
    let mut invalid_parent_order=0usize;
    let mut max_depth=0usize;
    let mut unknown_dos_time=0usize;
    let mut group_counts=HashMap::<(String,u64),u64>::new();
    let mut disks=Vec::with_capacity(c.disks.len());
    for (disk_number,d) in c.disks.iter().enumerate() {
        let mut paths=Vec::<String>::with_capacity(d.entries.len());
        let mut depths=Vec::<usize>::with_capacity(d.entries.len());
        let mut file_count=0u64;let mut directory_count=0u64;let mut archive_count=0u64;
        for (i,e) in d.entries.iter().enumerate() {
            match e.kind {
                EntryKind::File=>{file_count+=1;incr(&mut kinds,"file");},
                EntryKind::Directory=>{directory_count+=1;incr(&mut kinds,"directory");},
                EntryKind::Archive=>{archive_count+=1;incr(&mut kinds,"archive");},
            }
            incr(&mut sizes,size_bucket(e.size));
            incr(&mut desc_lengths,byte_bucket(e.description.len()));
            incr(&mut raw_lengths,byte_bucket(e.opaque.len()));
            let attr_key=if e.attributes==0 {"zero"} else if e.attributes==16 {"directory_16"}
                else if e.attributes==32 {"archive_32"} else {"other"};
            incr(&mut attr,attr_key);
            name_bytes+=e.name.len() as u64;
            desc_bytes+=e.description.len() as u64;
            raw_bytes+=e.opaque.len() as u64;
            max_raw=max_raw.max(e.opaque.len());
            longest_name=longest_name.max(e.name.len());
            if !e.name.is_ascii(){non_ascii+=1}
            if e.modified_dos==0 {unknown_dos_time+=1}
            if e.kind==EntryKind::File {
                let extension=Path::new(&e.name).extension().and_then(|s|s.to_str())
                    .unwrap_or("").to_lowercase();
                *extensions.entry(extension).or_default()+=1;
                *group_counts.entry((e.name.to_lowercase(),e.size)).or_default()+=1;
            }
            let (path,depth)=match e.parent {
                None=>{roots+=1;(e.name.clone(),0usize)},
                Some(p) if p<i=>(format!("{}/{}",paths[p],e.name),depths[p]+1),
                _=>{invalid_parent_order+=1;(e.name.clone(),0usize)},
            };
            path_bytes+=path.len() as u64;
            longest_path=longest_path.max(path.len());
            max_depth=max_depth.max(depth);
            paths.push(path);depths.push(depth);
        }
        disks.push(json!({"ordinal":disk_number+1,"entries":d.entries.len(),
                           "files":file_count,"directories":directory_count,
                           "archives":archive_count,"raw_header_bytes":d.opaque_header.len()}));
    }
    let groups=group_counts.values().filter(|&&count|count>1).count();
    let duplicate_members=group_counts.values().filter(|&&count|count>1).sum::<u64>();
    let mut top_extensions=extensions.into_iter().collect::<Vec<_>>();
    top_extensions.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let other_extensions=top_extensions.iter().skip(15).map(|x|x.1).sum::<u64>();
    let top_extensions=top_extensions.into_iter().take(15)
        .map(|(extension,count)|json!({"extension":extension,"count":count}))
        .collect::<Vec<_>>();
    json!({
        "counts":{"disks":c.disks.len(),"entries":c.disks.iter().map(|d|d.entries.len() as u64).sum::<u64>(),
                  "roots":roots,"non_ascii_names":non_ascii,"invalid_parent_order":invalid_parent_order,
                  "zero_modified_dos":unknown_dos_time},
        "disks_without_labels":disks,"kinds":kinds,"attributes_coarse":attr,
        "file_sizes_bytes_buckets":sizes,"description_bytes_buckets":desc_lengths,
        "ctf_opaque_length_buckets":raw_lengths,
        "bytes":{"stored_name_utf8":name_bytes,"materialized_full_paths_utf8":path_bytes,
                 "decoded_descriptions_utf8":desc_bytes,"raw_opaque_records":raw_bytes,
                 "max_opaque_record":max_raw,"longest_name_utf8":longest_name,
                 "longest_materialized_path_utf8":longest_path,"trailer":c.opaque_trailer.len()},
        "hierarchy":{"max_depth":max_depth,"invalid_parent_order":invalid_parent_order},
        "duplicates_name_size":{"groups":groups,"members":duplicate_members,
                                  "content_hash_verified":false},
        "top_extensions":top_extensions,"other_extension_entries":other_extensions,
        "statistics_ms":started.elapsed().as_secs_f64()*1000.0,
        "rss_after_statistics":rss(),
        "privacy":"No original names, descriptions, paths or raw record bytes disclosed; extensions and aggregate counts only"
    })
}
fn probe(path:&Path)->Value {
    let t=Instant::now();
    let snap=match capture(path,2_000_000_000) {
        Ok(s)=>s,
        Err(e)=>return json!({"status":"read_failed","error_kind":format!("{:?}",e.kind()),
                "no_source_mutation":true,"contains_user_content":false}),
    };
    let double_capture_ms=t.elapsed().as_secs_f64()*1000.0;
    let t=Instant::now();
    let c=match ctf_format::parse_ctf(snap.bytes(),&Limits::default()) {
        Ok(c)=>c,
        Err(e)=>return json!({"status":"parser_failed","source":snap.identity,
             "offset":e.offset,"capture_ms":double_capture_ms,"parser_ms":t.elapsed().as_secs_f64()*1000.0,
             "no_source_mutation":true,"content_disclosed":false}),
    };
    let parse_ms=t.elapsed().as_secs_f64()*1000.0;
    let t=Instant::now();
    let statistics=summarize(&c);
    let aggregate_ms=t.elapsed().as_secs_f64()*1000.0;
    let diagnostic=match ctf_format::diagnostics(&c) {
        Ok(d)=>json!({"status":"ok","archived_files":d.archived_files,
                      "archive_containers":d.archive_containers,
                      "descriptions":d.descriptions,
                      "unsupported_descriptions":d.unsupported_descriptions}),
        Err(_)=>json!({"status":"diagnostics_failed","raw_records_preserved_in_memory":true}),
    };
    let t=Instant::now();
    let encoded=ctf_format::write_ctf(&c);
    let encode_ms=t.elapsed().as_secs_f64()*1000.0;
    let roundtrip=match encoded {
        Ok(data)=>{
            let equal=data==snap.bytes();
            let first_difference=if equal {None}else{
                data.iter().zip(snap.bytes()).position(|(a,b)|a!=b)
                    .or(Some(data.len().min(snap.bytes().len())))
            };
            json!({"status":if equal {"byte_identical"}else{"byte_difference"},
                  "input_bytes":snap.bytes().len(),"output_bytes":data.len(),
                  "first_mismatch_offset":first_difference,
                  "output_sha256":sha256(&data),"encode_ms":encode_ms})
        },
        Err(e)=>json!({"status":"writer_failed","offset":e.offset,"encode_ms":encode_ms}),
    };
    let after=match capture(path,2_000_000_000) {
        Ok(s)=>json!({"source_unchanged":s.identity==snap.identity}),
        Err(_)=>json!({"source_unchanged":"could_not_revalidate"}),
    };
    json!({
        "status":"parsed","source":snap.identity,"capture_double_hash_ms":double_capture_ms,
        "parse_ms":parse_ms,"aggregate_ms":aggregate_ms,"statistics":statistics,
        "parser_diagnostics":diagnostic,"ctf_roundtrip":roundtrip,
        "post_check":after,"memory":rss(),
        "original_ctf_read_only":true,
        "uncertainties":["Roundtrip byte identity is stricter than semantic equality",
                         "Original WhereIsIt opening modified exports not tested",
                         "No cold-cache or p95 latency measured"]
    })
}
fn self_test()->bool {
    let d=tempfile::tempdir().unwrap();
    let original=d.path().join("fixture.ctf");
    let c=CtfCatalog{name:"B1 synthetic fixture".into(),opaque_trailer:Vec::new(),
        disks:vec![CtfDisk{name:"Disk".into(),opaque_header:Vec::new(),
            entries:vec![
                CtfEntry{name:"Каталог".into(),description:String::new(),parent:None,
                    kind:EntryKind::Directory,size:0,attributes:16,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
                CtfEntry{name:"a.pdf".into(),description:"Описание".into(),parent:Some(0),
                    kind:EntryKind::File,size:100,attributes:32,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
                CtfEntry{name:"a.pdf".into(),description:String::new(),parent:Some(0),
                    kind:EntryKind::File,size:100,attributes:32,
                    modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()},
            ]}]};
    let bytes=ctf_format::write_ctf(&c).unwrap();
    fs::write(&original,bytes).unwrap();
    let r=probe(&original);
    assert_eq!(r["status"],"parsed");
    assert_eq!(r["statistics"]["counts"]["entries"],3);
    assert_eq!(r["statistics"]["duplicates_name_size"]["members"],2);
    assert_eq!(r["statistics"]["counts"]["non_ascii_names"],1);
    assert_eq!(r["post_check"]["source_unchanged"],true);
    println!("{}",json!({"self_test":"PASS","synthetic":true,
        "roundtrip":r["ctf_roundtrip"]["status"],"source_read_only":true}));
    true
}
fn main() {
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    if args==["--self-test"] {self_test();return;}
    if args.is_empty(){eprintln!("Usage: ctf_real_probe <CTF1> [<CTF2> ...] | --self-test");std::process::exit(2);}
    let mut reports=Vec::new();
    let mut passed=true;
    for (i,path) in args.iter().enumerate() {
        let r=probe(Path::new(path));
        if r["status"]!="parsed" || r["post_check"]["source_unchanged"]!=true {passed=false;}
        reports.push(json!({"dataset_number":i+1,"report":r}));
    }
    println!("{}",serde_json::to_string_pretty(&json!({
        "tool":"ctf_real_probe_v1","datasets":reports,
        "actual_input_paths_in_report":false,"private_bytes_in_report":false
    })).unwrap());
    if !passed{std::process::exit(1)}
}
