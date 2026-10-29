#!/usr/bin/env python3
"""Synthetic CTF-derived canonical rows, identical ordered-ID pages and counts.
Each engine runs in its own process. No private CTF, SQL database or names are
uploaded by the accompanying workflow. Baseline app queries return full Entries;
all engines in this file return IDs only. Do not conflate these response shapes.
"""
from __future__ import annotations
import argparse, hashlib, json, os, platform, resource, sqlite3, statistics, time
from pathlib import Path

NAMES = ['ext-size','name-substring','broad-path-ext','sparse-description',
         'negative-description-ext','inherited-category-ext','or-cross-field',
         'no-match','short-text','selective-folder-ext','wildcard-name',
         'boolean-or-not-size','rare-large-size']

def memory():
    values = {'peak_rss_kib': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}
    try:
        for line in Path('/proc/self/status').read_text().splitlines():
            if line.startswith('VmRSS:'): values['rss_kib'] = int(line.split()[1])
    except OSError: pass
    return values

def measure(call, repeats=7):
    times=[]; result=None
    for _ in range(repeats):
        t=time.perf_counter(); result=call(); times.append((time.perf_counter()-t)*1000)
    return result, {'first_ms':times[0], 'median_ms':statistics.median(times[1:]),
                    'warm_samples_ms':times[1:], 'cache':'OS cache not flushed'}

def save(root, name, data):
    (root / (name+'.json')).write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'report':name,'memory':memory()},ensure_ascii=False),flush=True)

def where(q, sqlite=False):
    name='e.name_norm'; ext='e.ext'; size='e.size'
    path='unicode_lower(e.relative_path)' if sqlite else 'e.path_norm'
    category='e.volume_id<=6' if sqlite else 'e.category=1'
    def has(col,s): return f"instr({col},'{s}')>0"
    def desc(s):
        if sqlite:
            return f"e.id IN (SELECT entry_id FROM entry_payload WHERE instr(unicode_lower(description),'{s}')>0)"
        return has('e.description',s)
    queries=[
        f"{ext}='mkv' AND {size}>4294967296",
        has(name,'запись-0000200'),
        f"({has(path,'медиатека')}) AND {ext}='flac'",
        desc('редкое'),
        f"NOT ({desc('редкое')}) AND {ext}='mp3'",
        f"({category}) AND {ext}='mkv'",
        f"({has(name,'музыка')} OR {has(path,'музыка')} OR ({desc('музыка')})) OR {has(name,'0000042')}",
        has(name,'zzzzzznomatch'),
        has(name,'з'),
        f"{has(path,'album-0077')} AND {ext}='flac'",
        f"{has(name,'0000042')} AND substr({name},-4)='.mkv'",
        f"({ext}='mkv' OR {ext}='flac') AND NOT ({has(path,'album-0001')}) AND {size}>1073741824",
        f"{size}>9000000000",
    ]
    return queries[q]

def sql_bench(root, con, engine, table, oracle, sqlite=False):
    results=[]; order='e.name COLLATE NOCASE,e.id' if sqlite else 'e.name_norm,e.id'
    for q,label in enumerate(NAMES):
        predicate=where(q,sqlite)
        sql=f'SELECT e.id FROM {table} e WHERE {predicate} ORDER BY {order} LIMIT 250'
        values,timing=measure(lambda: con.execute(sql).fetchall())
        ids=[int(x[0]) for x in values]
        assert ids==oracle['queries'][q]['page_ids'], (engine,q,'page mismatch',ids[:10],oracle['queries'][q]['page_ids'][:10])
        t=time.perf_counter(); total=int(con.execute(f'SELECT count(*) FROM {table} e WHERE {predicate}').fetchone()[0]); count_ms=(time.perf_counter()-t)*1000
        assert total==oracle['queries'][q]['total'], (engine,q,total,oracle['queries'][q]['total'])
        plan=con.execute(('EXPLAIN QUERY PLAN ' if sqlite else 'EXPLAIN ')+sql).fetchall()
        results.append({'id':q,'case':label,'page':timing,'count_once_ms':count_ms,'total':total,'parity':True,'plan':plan})
        print(f'{engine} Q{q}: page={timing["median_ms"]:.3f} ms count={count_ms:.3f} ms',flush=True)
    t=time.perf_counter()
    groups,members=con.execute(f"SELECT COUNT(*),COALESCE(SUM(n),0) FROM (SELECT name_norm,size,COUNT(*) AS n FROM {table} WHERE kind='file' GROUP BY name_norm,size HAVING COUNT(*)>1)").fetchone()
    duplicate_ms=(time.perf_counter()-t)*1000
    assert int(groups)==oracle['duplicate_groups'] and int(members)==oracle['duplicate_members'],(engine,'duplicates')
    # Deep offset vs a continuation cursor at the SAME position and sort semantics.
    base=f"SELECT e.name,e.name_norm,e.id FROM {table} e WHERE e.ext='mkv' AND e.size>4294967296 ORDER BY {order}"
    cursor=con.execute(base+' LIMIT 1 OFFSET 49999').fetchone()
    paging=None
    if cursor:
        offset_sql=base+' LIMIT 250 OFFSET 50000'
        if sqlite:
            key_sql=f"SELECT e.name,e.name_norm,e.id FROM {table} e WHERE e.ext='mkv' AND e.size>4294967296 AND (e.name COLLATE NOCASE,e.id)>(?,?) ORDER BY {order} LIMIT 250"
            params=(cursor[0],cursor[2])
        else:
            key_sql=f"SELECT e.name,e.name_norm,e.id FROM {table} e WHERE e.ext='mkv' AND e.size>4294967296 AND (e.name_norm,e.id)>(?,?) ORDER BY {order} LIMIT 250"
            params=(cursor[1],cursor[2])
        offset_rows,off=measure(lambda:con.execute(offset_sql).fetchall())
        cursor_rows,key=measure(lambda:con.execute(key_sql,params).fetchall())
        assert offset_rows==cursor_rows,(engine,'cursor page mismatch')
        paging={'offset_50000':off,'keyset_after_cursor':key,'cursor_preparation_excluded':True,'same_page':True}
    _,point=measure(lambda:con.execute(f'SELECT name,size FROM {table} WHERE id=1000').fetchall())
    return {'engine':engine,'shape':'ID-only ordered first 250, count separately', 'queries':results,
            'duplicates':{'groups':int(groups),'members':int(members),'first_build_ms':duplicate_ms},
            'paging':paging,'point_lookup':point,'memory':memory()}

def prepare(root):
    import duckdb
    target=root/'analytical.duckdb'; assert not target.exists()
    con=duckdb.connect(str(target),config={'threads':2,'memory_limit':'2GB'})
    columns={k:t for k,t in zip(['id','volume_id','parent_id','kind','name','name_norm','path','path_norm','ext','size','description','category'],
                               ['UINTEGER','UINTEGER','UINTEGER','VARCHAR','VARCHAR','VARCHAR','VARCHAR','VARCHAR','VARCHAR','UBIGINT','VARCHAR','UTINYINT'])}
    t=time.perf_counter()
    con.execute(f"CREATE TABLE catalog AS SELECT * FROM read_csv(?, delim='\t', header=true, columns={columns!r}, nullstr='__NULL_NEVER__')",[str(root/'canonical.tsv')])
    import_s=time.perf_counter()-t
    export={}
    for name,select in [('canonical','SELECT * FROM catalog'),('sorted','SELECT * FROM catalog ORDER BY ext,name_norm,id')]:
        t=time.perf_counter(); path=str(root/(name+'.parquet')).replace("'","''")
        con.execute(f"COPY ({select}) TO '{path}' (FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 122880)")
        export[name]={'seconds':time.perf_counter()-t,'bytes':(root/(name+'.parquet')).stat().st_size}
    t=time.perf_counter();con.execute('CREATE INDEX id_lookup ON catalog(id)');index_s=time.perf_counter()-t
    con.execute('CHECKPOINT');con.close()
    save(root,'columnar-prepare',{'duckdb_version':duckdb.__version__,'threads':2,'import_tsv_s':import_s,'point_index_build_s':index_s,'duckdb_bytes':target.stat().st_size,'parquet':export,'canonical_has_raw_ctf_provenance':False,'warning':'These derived analytical files omit raw CTF bytes. Add original CTF size before comparing durable footprints.','memory':memory()})

def sqlite_run(root,oracle,in_memory=False):
    path=root/'released-v3.sqlite'
    source=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True)
    if in_memory:
        t=time.perf_counter();con=sqlite3.connect(':memory:');source.backup(con);load_s=time.perf_counter()-t;source.close()
    else:
        source.close();con=sqlite3.connect(path);load_s=None
    con.create_function('unicode_lower',1,lambda x:(x or '').lower(),deterministic=True)
    con.execute('PRAGMA cache_size=-65536');con.execute('PRAGMA temp_store=MEMORY')
    if not in_memory:
        report=sql_bench(root,con,'sqlite-direct','entries_core',oracle,True)
        report['sqlite_version']=sqlite3.sqlite_version;save(root,'sqlite-direct',report)
        t=time.perf_counter()
        con.execute('CREATE INDEX perf_ext_order_size ON entries_core(ext,name COLLATE NOCASE,id,size)')
        con.execute('ANALYZE entries_core');con.commit();index_s=time.perf_counter()-t
    else: index_s=None
    report=sql_bench(root,con,'sqlite-ram-tuned' if in_memory else 'sqlite-tuned','entries_core',oracle,True)
    report.update({'sqlite_version':sqlite3.sqlite_version,'copy_to_ram_s':load_s,'index_build_s':index_s,
                   'index_note':'Adds ext + name order + id + size, preserves existing indexes; direct predicates, no FTS candidate stage; category uses known synthetic inherited membership',
                   'python_udf_note':'Unicode lowercase for paths runs in Python here, unlike the production Rust UDF'})
    save(root,'sqlite-ram-tuned' if in_memory else 'sqlite-tuned',report);con.close()

def duck_run(root,oracle,mode):
    import duckdb
    t=time.perf_counter()
    if mode=='duck-file':
        con=duckdb.connect(str(root/'analytical.duckdb'),read_only=True,config={'threads':2,'memory_limit':'2GB'});table='catalog'
    else:
        con=duckdb.connect(config={'threads':2,'memory_limit':'2GB'})
        path=str(root/('sorted.parquet' if mode=='duck-sorted' else 'canonical.parquet')).replace("'","''")
        if mode=='duck-ram':
            con.execute(f"CREATE TABLE catalog AS SELECT * FROM read_parquet('{path}')");con.execute('CREATE INDEX id_lookup ON catalog(id)');table='catalog'
        else: table=f"read_parquet('{path}')"
    load_s=time.perf_counter()-t
    report=sql_bench(root,con,mode,table,oracle)
    report.update({'duckdb_version':duckdb.__version__,'threads':2,'open_and_load_s':load_s})
    save(root,mode,report);con.close()

def polars_run(root,oracle,eager):
    import polars as pl
    t=time.perf_counter(); path=root/'canonical.parquet'
    base=pl.read_parquet(path).lazy() if eager else pl.scan_parquet(path)
    load_s=time.perf_counter()-t
    name=pl.col('name_norm');path_expr=pl.col('path_norm');desc=pl.col('description');ext=pl.col('ext');size=pl.col('size')
    def contains(expr,s):return expr.str.contains(s,literal=True)
    expressions=[
        (ext=='mkv')&(size>4294967296),contains(name,'запись-0000200'),
        contains(path_expr,'медиатека')&(ext=='flac'),contains(desc,'редкое'),
        (~contains(desc,'редкое'))&(ext=='mp3'),(pl.col('category')==1)&(ext=='mkv'),
        contains(name,'музыка')|contains(path_expr,'музыка')|contains(desc,'музыка')|contains(name,'0000042'),
        contains(name,'zzzzzznomatch'),contains(name,'з'),contains(path_expr,'album-0077')&(ext=='flac'),
        contains(name,'0000042')&name.str.ends_with('.mkv'),
        ext.is_in(['mkv','flac'])&(~contains(path_expr,'album-0001'))&(size>1073741824),size>9000000000]
    results=[];engine='polars-ram' if eager else 'polars-parquet'
    for q,(label,expr) in enumerate(zip(NAMES,expressions)):
        plan=base.filter(expr).sort(['name_norm','id']).select('id').limit(250)
        ids,timing=measure(lambda:plan.collect(engine='streaming')['id'].to_list())
        assert ids==oracle['queries'][q]['page_ids'],(engine,q,'page mismatch')
        t=time.perf_counter();total=base.filter(expr).select(pl.len()).collect(engine='streaming').item();count_ms=(time.perf_counter()-t)*1000
        assert total==oracle['queries'][q]['total'],(engine,q,'count mismatch')
        results.append({'id':q,'case':label,'page':timing,'count_once_ms':count_ms,'total':total,'parity':True,'plan':plan.explain()})
        print(f'{engine} Q{q}: {timing["median_ms"]:.3f} ms',flush=True)
    t=time.perf_counter()
    result=base.filter(pl.col('kind')=='file').group_by(['name_norm','size']).len().filter(pl.col('len')>1).select(groups=pl.len(),members=pl.col('len').sum()).collect(engine='streaming').row(0)
    duplicate_ms=(time.perf_counter()-t)*1000
    assert result==(oracle['duplicate_groups'],oracle['duplicate_members']),(engine,'duplicate mismatch',result)
    save(root,engine,{'engine':engine,'polars_version':pl.__version__,'threads':pl.thread_pool_size(),'open_and_load_s':load_s,'queries':results,'duplicates':{'groups':result[0],'members':result[1],'first_build_ms':duplicate_ms},'memory':memory()})

def main():
    ap=argparse.ArgumentParser();ap.add_argument('mode');ap.add_argument('directory',type=Path);args=ap.parse_args();root=args.directory.resolve()
    assert (root/'generate.json').is_file(), 'This harness requires its own synthetic generator receipt'
    receipt=json.loads((root/'generate.json').read_text());assert receipt['dataset'].startswith('SYNTHETIC')
    oracle=json.loads((root/'ram.json').read_text())
    if args.mode=='prepare': prepare(root)
    elif args.mode in ('sqlite','sqlite-ram'):sqlite_run(root,oracle,args.mode=='sqlite-ram')
    elif args.mode.startswith('duck-'):duck_run(root,oracle,args.mode)
    elif args.mode in ('polars-ram','polars-parquet'):polars_run(root,oracle,args.mode=='polars-ram')
    else:raise ValueError('unknown mode')
if __name__=='__main__': main()
