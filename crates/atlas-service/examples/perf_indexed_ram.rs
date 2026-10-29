//! Synthetic experimental engine: full match bitsets + first page + exact counts.
//! Not a production query parser or an editable catalog implementation.
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::HashMap,fs,path::Path,time::Instant};
#[derive(Clone)]
struct Bits{words:Vec<u64>,n:usize}
impl Bits{
 fn empty(n:usize)->Self{Self{words:vec![0;n.div_ceil(64)],n}}
 fn add(&mut self,rank:u32){self.words[rank as usize/64]|=1u64<<(rank%64);}
 fn or(mut self,b:&Self)->Self{for(a,b)in self.words.iter_mut().zip(&b.words){*a|=*b;}self}
 fn and(mut self,b:&Self)->Self{for(a,b)in self.words.iter_mut().zip(&b.words){*a&=*b;}self}
 fn minus(mut self,b:&Self)->Self{for(a,b)in self.words.iter_mut().zip(&b.words){*a&=!*b;}self}
 fn count(&self)->u64{self.words.iter().map(|x|x.count_ones()as u64).sum()}
 fn visit<F:FnMut(u32)->bool>(&self,mut visit:F){for(wi,&word)in self.words.iter().enumerate(){let mut w=word;while w!=0{let bit=w.trailing_zeros();let rank=wi*64+bit as usize;if rank<self.n&&!visit(rank as u32){return;}w&=w-1;}}}
 fn page(&self,order:&[u32],offset:usize)->Vec<u32>{let mut page=Vec::with_capacity(250);let mut skip=offset;self.visit(|rank|{if skip>0{skip-=1;return true;}page.push(order[rank as usize]+1);page.len()<250});page}
}
struct Doc{text:String,ranks:Vec<u32>}
struct TextIndex{docs:Vec<Doc>,grams:HashMap<[u8;3],Vec<u32>>,n:usize}
impl TextIndex{
 fn build(docs:Vec<Doc>,n:usize)->Self{
  let mut grams:HashMap<[u8;3],Vec<u32>>=HashMap::new();
  for(i,doc)in docs.iter().enumerate(){let mut keys:Vec<[u8;3]>=doc.text.as_bytes().windows(3).map(|w|[w[0],w[1],w[2]]).collect();keys.sort_unstable();keys.dedup();for key in keys{grams.entry(key).or_default().push(i as u32);}}
  Self{docs,grams,n}
 }
 fn contains(&self,needle:&str)->Bits{
  let mut result=Bits::empty(self.n);let mut shortest:Option<&Vec<u32>>=None;
  for bytes in needle.as_bytes().windows(3){let key=[bytes[0],bytes[1],bytes[2]];let Some(list)=self.grams.get(&key)else{return result;};if shortest.is_none_or(|v|list.len()<v.len()){shortest=Some(list);}}
  let mut add=|d:&Doc|{if d.text.contains(needle){for &rank in &d.ranks{result.add(rank);}}};
  if let Some(list)=shortest{for &di in list{add(&self.docs[di as usize]);}}else{for d in &self.docs{add(d);}}
  result
 }
 fn capacity_bytes(&self)->usize{self.docs.capacity()*std::mem::size_of::<Doc>()+self.docs.iter().map(|d|d.text.capacity()+d.ranks.capacity()*4).sum::<usize>()+self.grams.values().map(|v|v.capacity()*4).sum::<usize>()}
}
#[derive(Clone,Copy)]
struct Row{size:u64,name:u32,parent:u32,volume:u32,ext:u8,file:bool}
struct Index{rows:Vec<Row>,order:Vec<u32>,names:TextIndex,paths:TextIndex,descriptions:TextIndex,extensions:Vec<Bits>,featured:Bits,sizes:Vec<(u64,u32)>}
impl Index{
 fn path(&self,s:&str)->Bits{assert!(!s.contains('/'),"boundary-spanning paths are outside this test corpus");self.names.contains(s).or(&self.paths.contains(s))}
 fn size_gt(&self,size:u64)->Bits{let p=self.sizes.partition_point(|v|v.0<=size);let mut set=Bits::empty(self.rows.len());for &(_,rank)in &self.sizes[p..]{set.add(rank);}set}
 fn query(&self,q:usize)->Bits{match q{
  0=>self.extensions[0].clone().and(&self.size_gt(4_294_967_296)),
  1=>self.names.contains("запись-0000200"),
  2=>self.path("медиатека").and(&self.extensions[1]),
  3=>self.descriptions.contains("редкое"),
  4=>self.extensions[2].clone().minus(&self.descriptions.contains("редкое")),
  5=>self.featured.clone().and(&self.extensions[0]),
  6=>self.path("музыка").or(&self.descriptions.contains("музыка")).or(&self.names.contains("0000042")),
  7=>self.names.contains("zzzzzznomatch"),
  8=>self.names.contains("з"),
  9=>self.path("album-0077").and(&self.extensions[1]),
  10=>self.names.contains("0000042").and(&self.extensions[0]),
  11=>self.extensions[0].clone().or(&self.extensions[1]).minus(&self.path("album-0001")).and(&self.size_gt(1_073_741_824)),
  12=>self.size_gt(9_000_000_000),_=>unreachable!()}}
}
fn rss()->Value{let s=fs::read_to_string("/proc/self/status").unwrap_or_default();let get=|key:&str|s.lines().find(|x|x.starts_with(key)).and_then(|x|x.split_whitespace().nth(1)).and_then(|v|v.parse::<u64>().ok());json!({"rss_kib":get("VmRSS:"),"peak_rss_kib":get("VmHWM:")})}
fn intern(map:&mut HashMap<String,u32>,docs:&mut Vec<Doc>,s:&str)->u32{if let Some(&id)=map.get(s){return id;}let id=docs.len()as u32;map.insert(s.into(),id);docs.push(Doc{text:s.into(),ranks:Vec::new()});id}
fn main(){
 let args:Vec<String>=std::env::args().collect();assert_eq!(args.len(),2);let dir=Path::new(&args[1]);
 let receipt:Value=serde_json::from_slice(&fs::read(dir.join("generate.json")).unwrap()).unwrap();assert!(receipt["dataset"].as_str().unwrap().starts_with("SYNTHETIC"));
 let oracle:Value=serde_json::from_slice(&fs::read(dir.join("ram.json")).unwrap()).unwrap();let n=oracle["rows"].as_u64().unwrap()as usize;
 let t=Instant::now();let mut rows=Vec::with_capacity(n);let mut names=Vec::new();let mut name_map=HashMap::new();let mut folders:HashMap<u32,String>=HashMap::new();let mut desc_by_row:HashMap<u32,u32>=HashMap::new();let mut descriptions=Vec::new();let mut desc_map=HashMap::new();
 let mut reader=csv::ReaderBuilder::new().delimiter(b'\t').from_path(dir.join("canonical.tsv")).unwrap();
 for record in reader.records(){let r=record.unwrap();let id:u32=r[0].parse().unwrap();assert_eq!(id as usize,rows.len()+1);
  let name=intern(&mut name_map,&mut names,&r[5]);let file=&r[3]=="file";let ext=match &r[8]{"mkv"=>0,"flac"=>1,"mp3"=>2,"txt"=>3,_=>4};
  rows.push(Row{name,parent:r[2].parse().unwrap(),volume:r[1].parse().unwrap(),size:r[9].parse().unwrap(),ext,file});
  if !file{folders.insert(id,r[7].into());}if !r[10].is_empty(){let di=intern(&mut desc_map,&mut descriptions,&r[10]);desc_by_row.insert(id,di);}
 }
 assert_eq!(rows.len(),n);drop(reader);drop(name_map);drop(desc_map);
 let load_s=t.elapsed().as_secs_f64();let t=Instant::now();let mut order:Vec<u32>=(0..n as u32).collect();order.sort_unstable_by(|&a,&b|names[rows[a as usize].name as usize].text.cmp(&names[rows[b as usize].name as usize].text).then(a.cmp(&b)));
 let mut extensions:Vec<Bits>=(0..5).map(|_|Bits::empty(n)).collect();let mut featured=Bits::empty(n);let mut sizes=Vec::with_capacity(n);let mut members:HashMap<u32,Vec<u32>>=HashMap::new();
 for(rank,&idx)in order.iter().enumerate(){let r=rows[idx as usize];let rank=rank as u32;names[r.name as usize].ranks.push(rank);extensions[r.ext as usize].add(rank);if r.volume<=6{featured.add(rank);}sizes.push((r.size,rank));if r.parent!=0{members.entry(r.parent).or_default().push(rank);}if !r.file{members.entry(idx+1).or_default().push(rank);}if let Some(&di)=desc_by_row.get(&(idx+1)){descriptions[di as usize].ranks.push(rank);}}
 drop(desc_by_row);sizes.sort_unstable();let mut path_map=HashMap::new();let mut paths=Vec::new();for(id,text)in folders{let di=intern(&mut path_map,&mut paths,&text);if let Some(ranks)=members.remove(&id){paths[di as usize].ranks.extend(ranks);}}drop(path_map);drop(members);
 let names=TextIndex::build(names,n);let paths=TextIndex::build(paths,n);let descriptions=TextIndex::build(descriptions,n);let index_build_s=t.elapsed().as_secs_f64();
 let index=Index{rows,order,names,paths,descriptions,extensions,featured,sizes};let built_memory=rss();
 let mut reports=Vec::new();for q in 0..13{let mut samples=Vec::new();let mut output=(Vec::new(),0);for _ in 0..7{let t=Instant::now();let result=std::hint::black_box(index.query(q));output=(result.page(&index.order,0),result.count());std::hint::black_box(&output);samples.push(t.elapsed().as_secs_f64()*1000.0);}
  assert_eq!(json!(output.0),oracle["queries"][q]["page_ids"],"indexed page Q{q}");assert_eq!(json!(output.1),oracle["queries"][q]["total"],"indexed count Q{q}");let full=index.query(q);let mut hash=Sha256::new();full.visit(|rank|{hash.update((index.order[rank as usize]+1).to_le_bytes());true});let mut warm=samples[1..].to_vec();warm.sort_by(f64::total_cmp);let median=(warm[2]+warm[3])/2.0;
  eprintln!("Indexed RAM Q{q}: full matches + first page + exact count {median:.6} ms");reports.push(json!({"id":q,"query":oracle["queries"][q]["query"],"median_ms":median,"first_ms":samples[0],"warm_samples_ms":samples[1..],"total":output.1,"all_ordered_ids_sha256":format!("{:x}",hash.finalize()),"parity":true}));}
 let t=Instant::now();let mut dup:Vec<(u32,u64)>=index.rows.iter().filter(|r|r.file).map(|r|(r.name,r.size)).collect();dup.sort_unstable();let mut groups=0u64;let mut members=0u64;let mut p=0;while p<dup.len(){let mut end=p+1;while end<dup.len()&&dup[end]==dup[p]{end+=1;}if end-p>1{groups+=1;members+=(end-p)as u64;}p=end;}let duplicate_s=t.elapsed().as_secs_f64();assert_eq!(json!(groups),oracle["duplicate_groups"]);assert_eq!(json!(members),oracle["duplicate_members"]);
 let named=index.names.capacity_bytes();let pathbytes=index.paths.capacity_bytes();let descbytes=index.descriptions.capacity_bytes();let vectorbytes=index.rows.capacity()*std::mem::size_of::<Row>()+index.order.capacity()*4+index.sizes.capacity()*std::mem::size_of::<(u64,u32)>()+(index.extensions.len()+1)*n.div_ceil(64)*8;
 let report=json!({"dataset":receipt["dataset"],"profile":receipt.get("profile"),"rows":n,"row_bytes":std::mem::size_of::<Row>(),"canonical_read_and_intern_s":load_s,"all_index_build_s":index_build_s,"unique_names":index.names.docs.len(),"unique_folder_paths":index.paths.docs.len(),"unique_descriptions":index.descriptions.docs.len(),"name_text_posting_capacity_bytes":named,"folder_text_posting_capacity_bytes":pathbytes,"description_text_posting_capacity_bytes":descbytes,"major_vector_capacity_bytes":vectorbytes,"hash_table_bucket_bytes_in_capacity":false,"raw_ctf_resident_in_this_process":false,"memory_after_build":built_memory,"memory_final":rss(),"queries":reports,"duplicate_groups":groups,"duplicate_members":members,"integer_key_duplicate_build_s":duplicate_s,"notes":["Experimental fixed Boolean expressions, not generic production parser","Measures complete match sets, exact count and first 250 IDs together; other engines measured first page separately","Trigrams index distinct normalized names/folder paths/descriptions; candidate verification prevents false positives","Dense bitmaps ordered by filename rank; no Roaring implementation was benchmarked","Known synthetic category membership; arbitrary category updates and Unicode sort semantics remain separate acceptance tasks","Canonical TSV parsing is not direct zero-copy CTF ingestion; raw CTF must still be retained separately"]});
 fs::write(dir.join("indexed-ram.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
