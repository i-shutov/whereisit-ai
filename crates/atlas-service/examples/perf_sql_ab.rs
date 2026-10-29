//! Same native SQLite library, same hydrated Entry response, same generated CTF.
//! Separately measures original planner, covering index only, and fixed equivalent predicates.
use atlas_core::{Catalog,Entry};
use serde_json::{json,Value};
use std::{fs,path::Path,time::Instant};
fn timed<F:FnMut()->Vec<Entry>>(mut f:F)->(Vec<Entry>,Value){let mut samples=Vec::new();let mut last=Vec::new();for _ in 0..7{let t=Instant::now();last=std::hint::black_box(f());samples.push(t.elapsed().as_secs_f64()*1000.0);}let mut warm=samples[1..].to_vec();warm.sort_by(f64::total_cmp);(last,json!({"first_ms":samples[0],"median_ms":(warm[2]+warm[3])/2.0,"warm_samples_ms":samples[1..]}))}
fn full_sql(table:&str,filter:&str)->String{format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE {filter} ORDER BY name COLLATE NOCASE,id LIMIT 251) SELECT e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags FROM page p JOIN entries e ON e.id=p.id ORDER BY e.name COLLATE NOCASE,e.id")}
fn plan(c:&Catalog,sql:&str,args:&[rusqlite::types::Value])->Vec<String>{c.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap().query_map(rusqlite::params_from_iter(args.iter()),|r|r.get(3)).unwrap().collect::<Result<_,_>>().unwrap()}
fn main(){
 let args:Vec<String>=std::env::args().collect();assert_eq!(args.len(),2);let dir=Path::new(&args[1]);let receipt:Value=serde_json::from_slice(&fs::read(dir.join("generate.json")).unwrap()).unwrap();assert!(receipt["dataset"].as_str().unwrap().starts_with("SYNTHETIC"));
 let oracle:Value=serde_json::from_slice(&fs::read(dir.join("ram.json")).unwrap()).unwrap();let c=Catalog::open(dir.join("released-v3.sqlite")).unwrap();let mut reports=Vec::new();let mut expected=Vec::new();
 for q in 0..13{let label=oracle["queries"][q]["query"].as_str().unwrap();let(page,time)=timed(||c.search_fast(label,None,0,250).unwrap().items);let ids:Vec<_>=page.iter().map(|r|r.id).collect();assert_eq!(json!(ids),oracle["queries"][q]["page_ids"]);let(filter,params)=atlas_core::query::compile(label).unwrap();let p=plan(&c,&full_sql(c.query_table(label).unwrap(),&filter),&params);expected.push(json!(page));reports.push(json!({"id":q,"query":label,"original":time,"original_full_page_plan":p}));}
 let t=Instant::now();c.conn().execute_batch("CREATE INDEX perf_native_ext_order_size ON entries_core(ext,name COLLATE NOCASE,id,size); ANALYZE entries_core;").unwrap();let index_build_s=t.elapsed().as_secs_f64();
 for q in 0..13{let label=oracle["queries"][q]["query"].as_str().unwrap();let(page,time)=timed(||c.search_fast(label,None,0,250).unwrap().items);assert_eq!(json!(page),expected[q]);reports[q]["covering_index_only"]=time;
  let(original_filter,original_params)=atlas_core::query::compile(label).unwrap();reports[q]["indexed_original_full_page_plan"]=json!(plan(&c,&full_sql(c.query_table(label).unwrap(),&original_filter),&original_params));
  let filter=match q{
   0=>Some("ext='mkv' AND size>4294967296"),
   2=>Some("instr(unicode_lower(relative_path),'медиатека')>0 AND ext='flac'"),
   3=>Some("id IN (SELECT entry_id FROM entry_payload WHERE instr(unicode_lower(description),'редкое')>0)"),
   4=>Some("NOT (id IN (SELECT entry_id FROM entry_payload WHERE instr(unicode_lower(description),'редкое')>0)) AND ext='mp3'"),
   5=>Some("volume_id<=6 AND ext='mkv'"),
   6=>Some("instr(name_norm,'музыка')>0 OR instr(unicode_lower(relative_path),'музыка')>0 OR id IN (SELECT entry_id FROM entry_payload WHERE instr(unicode_lower(description),'музыка')>0) OR instr(name_norm,'0000042')>0"),
   11=>Some("(ext='mkv' OR ext='flac') AND instr(unicode_lower(relative_path),'album-0001')=0 AND size>1073741824"),
   12=>Some("size>9000000000"),_=>None};
  if let Some(filter)=filter{
   let sql=full_sql("entries_core",filter);
   let(page,time)=timed(||{let mut rows:Vec<Entry>=c.conn().prepare(&sql).unwrap().query_map([],atlas_core::entry_row).unwrap().collect::<Result<_,_>>().unwrap();if rows.len()>250{rows.pop();}rows});
   assert_eq!(json!(page),expected[q],"full Entry parity Q{q}");let count:u64=c.conn().query_row(&format!("SELECT COUNT(*) FROM entries_core WHERE {filter}"),[],|r|r.get(0)).unwrap();assert_eq!(json!(count),oracle["queries"][q]["total"]);
   reports[q]["equivalent_fixed_predicate"]=time;reports[q]["rewritten_full_page_plan"]=json!(plan(&c,&sql,&[]));reports[q]["complete_entry_page_and_count_equal"]=json!(true);
  }
  eprintln!("Native SQLite full Entry Q{q}: original={} ms index={} ms predicate={}",reports[q]["original"]["median_ms"],reports[q]["covering_index_only"]["median_ms"],reports[q]["equivalent_fixed_predicate"]["median_ms"]);
 }
 c.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
 fs::write(dir.join("native-sql-ab.json"),serde_json::to_vec_pretty(&json!({"dataset":receipt["dataset"],"rows":oracle["rows"],"native_sqlite":c.conn().query_row("SELECT sqlite_version()",[],|r|r.get::<_,String>(0)).unwrap(),"response_shape":"250 full Entry structs and continuation probe; same as production search_fast, exact full-field page equality asserted","covering_index_build_s":index_build_s,"sqlite_bytes_after_index":fs::metadata(dir.join("released-v3.sqlite")).unwrap().len(),"queries":reports,"caveats":["Fixed-predicate research, not a generic cost-based planner implementation","Q5 category rewritten to volume_id<=6 only because this synthetic category is exactly all descendants of those volume roots; arbitrary real categories require a membership index","Q6 fixed OR decomposition tested against this corpus; universal cross-field substring semantics need separate validation","First-run is not cold OS cache; no p95 SLA claimed","Source private CTF and production database are not modified"]})).unwrap()).unwrap();
}
