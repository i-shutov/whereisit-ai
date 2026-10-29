//! Anti-deduplication synthetic input: globally unique names and directory paths.
use ctf_format::{CtfCatalog,CtfDisk,CtfEntry,EntryKind};
use serde_json::json;
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::Path,time::Instant};
fn main(){
 let a:Vec<String>=std::env::args().collect();assert_eq!(a.len(),4);let dir=Path::new(&a[2]);fs::create_dir_all(dir).unwrap();let n:usize=a[3].parse().unwrap();assert!((1000..=3_100_000).contains(&n));let heavy=a[1]=="unique-payload";assert!(heavy||a[1]=="unique-3m");
 let t=Instant::now();let mut disks=Vec::new();let mut global=0;let mut directory_count=0;let mut described=0;
 for di in 0..44{
  let count=n/44+usize::from(di<n%44);let mut entries=Vec::with_capacity(count);let mut parent_folder=0;
  for local in 0..count{
   let key=global;global+=1;let directory=local==0||local%13==1;let ext=["mkv","flac","mp3","txt"][key%4];
   let name=if local==0{format!("Медиатека-диск-{di:02}")}else if directory{format!("album-{:04}-disk-{di:02}-Уникальный-раздел-音楽",local/13)}else{format!("Запись-{key:07}.{ext}")};
   let parent=if local==0{None}else if directory&&(local/13)%8==0{Some(0)}else{Some(parent_folder)};
   let size=if directory{0}else if key%10000==0{12_000_000_000+key as u64}else{[5_000_000_000,60_000_000,5_000_000,1000][key%4]+key as u64};
   let frequency=if heavy{5}else{100};let description=if !directory&&key%frequency==0{described+=1;if heavy{format!("редкое описание: музыка и кино запись-{key:07}. {}",format!("Метаданные-{key:07}-{}; ",key.wrapping_mul(2654435761usize)).repeat(80))}else{format!("редкое описание: музыка и кино запись-{key:07}, уникальные данные {}",key.wrapping_mul(2654435761usize))}}else{String::new()};
   entries.push(CtfEntry{name,description,parent,kind:if directory{EntryKind::Directory}else{EntryKind::File},size,attributes:if directory{16}else{32},modified_dos:0x58210000,created_dos:0x58210000,accessed_dos:0x58210000,opaque:Vec::new()});
   if directory{parent_folder=local;directory_count+=1;}
  }
  disks.push(CtfDisk{name:format!("Synthetic unique disk {di:02}"),entries,opaque_header:Vec::new()});
 }
 let c=CtfCatalog{name:format!("SYNTHETIC anti-dedup {}",a[1]),disks,opaque_trailer:Vec::new()};let generate_s=t.elapsed().as_secs_f64();let t=Instant::now();let bytes=ctf_format::write_ctf(&c).unwrap();let encode_s=t.elapsed().as_secs_f64();let t=Instant::now();let mut f=fs::OpenOptions::new().write(true).create_new(true).open(dir.join("source.ctf")).unwrap();f.write_all(&bytes).unwrap();f.sync_all().unwrap();
 fs::write(dir.join("generate.json"),serde_json::to_vec_pretty(&json!({"dataset":"SYNTHETIC anti-dedup stress, no private user data","profile":a[1],"rows":n,"volumes":44,"directories":directory_count,"described_entries":described,"unique_descriptions":described,"generator_s":generate_s,"ctf_encode_s":encode_s,"ctf_write_fsync_s":t.elapsed().as_secs_f64(),"ctf_bytes":bytes.len(),"ctf_sha256":format!("{:x}",Sha256::digest(&bytes)),"notes":"Globally unique file/folder names; directory every 13 rows; depth cycles at eight; descriptions vary per record"})).unwrap()).unwrap();
 println!("Generated {}: {n} rows, {directory_count} directories, {described} distinct descriptions",a[1]);
}
