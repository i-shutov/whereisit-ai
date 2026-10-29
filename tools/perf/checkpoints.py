#!/usr/bin/env python3
"""Synthetic persistence microbenchmarks. Process-crash tests are not power-loss proof."""
import gc,json,os,sqlite3,subprocess,sys,time
from pathlib import Path
import polars as pl
root=Path(sys.argv[1]).resolve()
assert json.loads((root/'generate.json').read_text())['dataset'].startswith('SYNTHETIC')
results={}
t=time.perf_counter();df=pl.read_parquet(root/'canonical.parquet');results['parquet_read_s']=time.perf_counter()-t
rows=df.height;expected_sum=int(df['size'].sum());expected_names=df['name'].to_list()[:250]
t=time.perf_counter();df.write_ipc(root/'hot-snapshot.arrow',compression='uncompressed');results['ipc_write_s']=time.perf_counter()-t
results['ipc_bytes']=(root/'hot-snapshot.arrow').stat().st_size
results['parquet_bytes']=(root/'canonical.parquet').stat().st_size
del df;gc.collect()
t=time.perf_counter();mapped=pl.read_ipc(root/'hot-snapshot.arrow',memory_map=True,rechunk=False);results['ipc_open_mmap_s']=time.perf_counter()-t
t=time.perf_counter();assert mapped.height==rows and int(mapped['size'].sum())==expected_sum and mapped['name'].head(250).to_list()==expected_names
results['ipc_first_numeric_touch_s']=time.perf_counter()-t
results['ipc_note']='Uncompressed Arrow file; open timing precedes complete data access. OS cache is not flushed. Original DataFrame allocation may remain in allocator caches. This is not a resident-memory comparison.'
del mapped;gc.collect()
path=root/'edits.sqlite';assert not path.exists()
con=sqlite3.connect(path);con.execute('PRAGMA journal_mode=WAL');con.execute('PRAGMA synchronous=FULL');con.execute('CREATE TABLE edits(seq INTEGER PRIMARY KEY,entry_id INTEGER NOT NULL,description TEXT NOT NULL)');con.commit()
t=time.perf_counter()
for i in range(100):
    con.execute('INSERT INTO edits VALUES(?,?,?)',(i,i+1,'Synthetic annotation'));con.commit()
results['journal_100_individual_commits_ms']=(time.perf_counter()-t)*1000
t=time.perf_counter()
with con:
    for i in range(100,1100):con.execute('INSERT INTO edits VALUES(?,?,?)',(i,i+1,'Synthetic batch annotation'))
results['journal_1000_edits_one_commit_ms']=(time.perf_counter()-t)*1000
assert con.execute('SELECT count(*) FROM edits').fetchone()[0]==1100
con.close()
# Child process exits without closing a transaction, then exits after a durable commit.
program='import os,sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute("PRAGMA synchronous=FULL"); c.execute("INSERT INTO edits VALUES(?,?,?)",(2000,1,"crash-test")); '+('c.commit(); ' if False else '')+'os._exit(23)'
r=subprocess.run([sys.executable,'-c',program,str(path)],check=False);assert r.returncode==23
con=sqlite3.connect(path);assert con.execute('SELECT count(*) FROM edits WHERE seq=2000').fetchone()[0]==0;con.close()
program=program.replace('os._exit(23)','c.commit(); os._exit(23)')
r=subprocess.run([sys.executable,'-c',program,str(path)],check=False);assert r.returncode==23
con=sqlite3.connect(path);assert con.execute('SELECT count(*) FROM edits WHERE seq=2000').fetchone()[0]==1;assert con.execute('PRAGMA integrity_check').fetchone()[0]=='ok';con.close()
results['process_exit_before_commit_rolled_back']=True
results['process_exit_after_commit_persisted']=True
results['power_failure_tested']=False
results['polars_version']=pl.__version__
(root/'snapshot-journal.json').write_text(json.dumps(results,indent=2),encoding='utf-8');print(json.dumps(results,indent=2))
