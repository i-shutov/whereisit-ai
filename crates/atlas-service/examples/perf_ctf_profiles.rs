//! Adverse synthetic profiles plus semantic verification of edited CTF saves.
use ctf_format::{CtfCatalog,CtfDisk,CtfEntry,EntryKind,Limits};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::Path,time::Instant};
fn put_new(p:&Path,b:&[u8]) {let mut f=fs::OpenOptions::new().write(true).create_new(true).open(p).unwrap();f.write_all(b).unwrap();f.sync_all().unwrap();}
fn sha(b:&[u8])->String {format!("{:x}",Sha256::digest(b))}
fn signature(c:&CtfCatalog)->Vec<String> {
    let mut result=Vec::new();
    for d in &c.disks {
        let mut paths:Vec<String>=Vec::new();
        for e in &d.entries {
            let p=e.parent.map(|x|format!("{}/{}",paths[x],e.name)).unwrap_or_else(||e.name.clone());
            result.push(json!([d.name,p,e.kind,e.size,e.attributes,e.modified_dos,e.created_dos,e.accessed_dos,e.description]).to_string());paths.push(p);
        }
    }
    result.sort_unstable();result
}
fn main(){
    let args:Vec<String>=std::env::args().collect();assert!(args.len()==4,"profile DIRECTORY COUNT");
    let profile=&args[1];assert!(matches!(profile.as_str(),"unique-deep"|"payload-heavy"));
    let dir=Path::new(&args[2]);fs::create_dir_all(dir).unwrap();let n:usize=args[3].parse().unwrap();assert!((1000..=1_000_000).contains(&n));
    let heavy=profile=="payload-heavy";let t=Instant::now();let mut disks=Vec::new();let mut global=0;
    for di in 0..44 {
        let count=n/44+usize::from(di<n%44);let mut entries=Vec::with_capacity(count);let mut folder=0;
        for local in 0..count {
            let is_dir=local==0||local%51==1;let key=global;global+=1;
            let ext=["mkv","flac","mp3","txt"][key%4];
            let name=if local==0 {"Медиатека".into()} else if is_dir {format!("album-{:04}-длинный-раздел-коллекции-音楽",local/51)} else {format!("Запись-{key:07}.{ext}")};
            let parent=if local==0 {None} else if is_dir && (local/51)%8==0 {Some(0)} else {Some(folder)};
            let size=if is_dir {0} else if key%10000==0 {12_000_000_000+key as u64} else {[5_000_000_000u64,60_000_000,5_000_000,1000][key%4]+key as u64};
            let description=if !is_dir && key%if heavy {5}else{100}==0 {format!("редкое описание: музыка и кино {}",if heavy {"Сведения о записи. ".repeat(100)}else {String::new()})}else{String::new()};
            entries.push(CtfEntry{name,description,parent,kind:if is_dir {EntryKind::Directory}else{EntryKind::File},size,attributes:if is_dir {16}else{32},modified_dos:0x58210000,created_dos:0x58210000,accessed_dos:0x58210000,opaque:Vec::new()});if is_dir{folder=local;}
        }
        disks.push(CtfDisk{name:format!("Synthetic profile disk {di}"),entries,opaque_header:Vec::new()});
    }
    let source=CtfCatalog{name:format!("SYNTHETIC profile {profile}"),disks,opaque_trailer:Vec::new()};
    let generate_s=t.elapsed().as_secs_f64();let t=Instant::now();let bytes=ctf_format::write_ctf(&source).unwrap();let encode_s=t.elapsed().as_secs_f64();
    let t=Instant::now();put_new(&dir.join("source.ctf"),&bytes);let fsync_s=t.elapsed().as_secs_f64();drop(source);
    let parsed=ctf_format::parse_ctf(&bytes,&Limits::default()).unwrap();let source_sha=sha(&bytes);
    fs::write(dir.join("generate.json"),serde_json::to_vec_pretty(&json!({"dataset":"SYNTHETIC adverse profile, not user data","profile":profile,"rows":n,"volumes":44,"generator_s":generate_s,"ctf_encode_s":encode_s,"ctf_write_fsync_s":fsync_s,"ctf_bytes":bytes.len(),"ctf_sha256":source_sha})).unwrap()).unwrap();
    let mut mutations:Vec<Value>=Vec::new();
    for mode in ["rename-file","rename-folder","change-description","append-file","remove-file"] {
        let mut edited=parsed.clone();
        let fi=edited.disks[0].entries.iter().position(|x|x.kind==EntryKind::File).unwrap();
        match mode {
            "rename-file"=>edited.disks[0].entries[fi].name="Изменённое имя 🎵.flac".into(),
            "rename-folder"=>edited.disks[0].entries[0].name="Переименованная медиатека 🎬".into(),
            "change-description"=>edited.disks[0].entries[fi].description="Изменённое описание без потери исходных байтов других записей".into(),
            "append-file"=>edited.disks[0].entries.push(CtfEntry{name:"Дополнение 🎵.flac".into(),description:"Новая запись".into(),parent:Some(0),kind:EntryKind::File,size:7_000_000_000,attributes:32,modified_dos:0x58210000,created_dos:0x58210000,accessed_dos:0x58210000,opaque:Vec::new()}),
            "remove-file"=>{let i=edited.disks[0].entries.iter().rposition(|e|e.kind==EntryKind::File).unwrap();assert!(edited.disks[0].entries[i+1..].is_empty());edited.disks[0].entries.remove(i);},
            _=>unreachable!()
        }
        let expected=signature(&edited);let t=Instant::now();let new=ctf_format::write_ctf(&edited).unwrap();let encode_ms=t.elapsed().as_secs_f64()*1000.0;
        let t=Instant::now();let path=dir.join(format!("mutation-{mode}.ctf"));put_new(&path,&new);let write_ms=t.elapsed().as_secs_f64()*1000.0;
        let t=Instant::now();let back=ctf_format::parse_ctf(&new,&Limits::default()).unwrap();assert_eq!(expected,signature(&back),"semantic CTF save {mode}");let verify_ms=t.elapsed().as_secs_f64()*1000.0;
        assert_eq!(sha(&fs::read(dir.join("source.ctf")).unwrap()),source_sha);
        mutations.push(json!({"case":mode,"encode_ms":encode_ms,"write_fsync_ms":write_ms,"read_parse_verify_ms":verify_ms,"bytes":new.len(),"sha256":sha(&new),"all_known_semantic_fields_equal":true,"source_unchanged":true,"original_WhereIsIt_acceptance":false}));fs::remove_file(path).unwrap();
    }
    fs::write(dir.join("persistence.json"),serde_json::to_vec_pretty(&json!({"profile":profile,"tests":mutations,"scope":"Synthetic self-codec semantic validation; not original-application compatibility and not crash-durability proof"})).unwrap()).unwrap();
    println!("Generated {profile}: {n} records; five edited CTF checkpoint cases passed");
}
