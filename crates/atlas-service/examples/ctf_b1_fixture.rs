//! Write a deterministic *synthetic* Catalog 3.00 fixture for private B1 CI.
 //! Not a parser and never reads or copies user-provided CTF files.
use ctf_format::{CtfCatalog,CtfDisk,CtfEntry,EntryKind};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
fn main(){
 let a:Vec<String>=std::env::args().skip(1).collect();
 if a.len()!=2{eprintln!("Usage: ctf_b1_fixture NEW_PATH COUNT");std::process::exit(2);}
 let path=Path::new(&a[0]);
 let n:usize=a[1].parse().expect("row count");
 assert!((100..=100_000).contains(&n));
 let mut disks=Vec::new();
 for disk in 0..2 {
  let mut entries=Vec::with_capacity(n/2+3);
  entries.push(CtfEntry{name:format!("Медиатека {disk}"),description:String::new(),parent:None,
   kind:EntryKind::Directory,size:0,attributes:16,modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new()});
  for i in 0..n/2 {
   let ext=["mp3","pdf","epub","flac"][i%4];
   entries.push(CtfEntry{
    name:format!("Запись-{i:07}.{ext}"),
    description:if i%7==0 {"Редкое описание: музыка и книги".into()}else{String::new()},
    parent:Some(0),kind:EntryKind::File,
    size:if i%25==0{5_000_000_000+i as u64}else{1024+i as u64},
    attributes:32,modified_dos:0,created_dos:0,accessed_dos:0,opaque:Vec::new(),
   });
  }
  disks.push(CtfDisk{name:format!("Synthetic disk {disk}"),entries,opaque_header:Vec::new()});
 }
 let catalog=CtfCatalog{name:"B1 SYNTHETIC — never from user input".into(),
  disks,opaque_trailer:Vec::new()};
 let data=ctf_format::write_ctf(&catalog).expect("writer");
 let mut output=OpenOptions::new().create_new(true).write(true).open(path).expect("new output");
 output.write_all(&data).expect("write");
 output.sync_all().expect("sync");
 println!("Synthetic CTF fixture written: {} bytes, approx {} records",data.len(),n+2);
}
