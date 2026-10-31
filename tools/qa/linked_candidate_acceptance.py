#!/usr/bin/env python3
"""Linux local-only persistent-service acceptance; no private paths/hashes in report.

Requires Python 3.13, DuckDB 1.4.1 and an exact-source atlas executable. Run only
on a local read-only CTF; all derivative catalogs and synthetic notes are made
in a temporary directory and removed. Reports allowlisted aggregates only.
This is not physical GUI, original-WhereIsIt or final hardware SLO acceptance.
"""
import argparse,csv,hashlib,json,math,selectors,sqlite3,subprocess,tempfile,time
from pathlib import Path


def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        while chunk:=f.read(4*1024*1024):h.update(chunk)
    return h.hexdigest()


def require(v,code):
    if not v:raise RuntimeError(code)


class Service:
    def __init__(self,cli):
        self.p=subprocess.Popen([str(cli),'serve'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',bufsize=1)
        self.s=selectors.DefaultSelector();self.s.register(self.p.stdout,selectors.EVENT_READ)
    def call(self,request,expected_error=None):
        start=time.perf_counter();self.p.stdin.write(json.dumps(request,ensure_ascii=False)+'\n');self.p.stdin.flush()
        require(self.s.select(1800),'response_timeout')
        reply=json.loads(self.p.stdout.readline());ms=(time.perf_counter()-start)*1000
        if expected_error is not None:
            require(reply.get('ok') is False and reply.get('error',{}).get('code')==expected_error,'wrong_rejection_'+request['op'])
            return {},ms
        if reply.get('ok') is not True:
            code=reply.get('error',{}).get('code','unknown')
            safe=code if isinstance(code,str) and code.replace('_','').isalnum() else 'unknown'
            raise RuntimeError('api_'+request['op']+'_'+safe)
        return reply['data'],ms
    def close(self):
        self.p.stdin.close()
        try:self.p.wait(timeout=5)
        except subprocess.TimeoutExpired:self.p.kill();self.p.wait()
        self.s.close()


def run(cli,source,work):
    before=sha(source);cache=work/'cache';notes=work/'local-test.wia.sqlite'
    client=Service(cli);report={'format':'whereisit-linked-candidate-qa-v1','status':'failed','private_content_in_report':False,'checks':{}}
    try:
        base={'path':str(source),'cache_root':str(cache)}
        sync,ms=client.call({'op':'ctf_sync',**base});g=sync['generation'];base['generation']=g
        require(sync['original_ctf_read_only'] is True,'sync_not_readonly')
        report.update(entries=sync['entries'],volumes=sync['volumes'],initial_sync_ms=round(ms,2))
        mirror=cache/g/'catalog.sqlite';mirror_before=sha(mirror)
        def call(op,expected_error=None,**kw):
            data,t=client.call({'op':op,**base,**kw},expected_error)
            if expected_error is None:require(data.get('source_sha_checked') is True,'missing_source_verification')
            return data.get('result'),t
        disks,_=call('ctf_volumes_pinned');require(len(disks)==sync['volumes'],'volume_count')
        children,_=call('ctf_children_pinned',volume_id=disks[0]['id'],limit=25)
        require(isinstance(children['items'],list),'folder_page_schema')
        report['checks']['read_only_volume_and_folder_browse']='PASS'
        first_entry=None;metrics=[]
        for i,q in enumerate(['ext:mp3','ext:mkv','NOT ext:tmp']):
            fast,t=call('ctf_search_pinned',query=q,limit=100,offset=0)
            full,_=call('ctf_search_pinned',query=q,limit=100,offset=0,full_count=True)
            ids=lambda a:[x['id'] for x in a['items']]
            require(ids(fast)==ids(full),'first_page_mismatch')
            require(fast['has_more']==(full['total']>100),'first_has_more')
            if full['total']>100:
                f2,_=call('ctf_search_pinned',query=q,limit=100,offset=100)
                c2,_=call('ctf_search_pinned',query=q,limit=100,offset=100,full_count=True)
                require(ids(f2)==ids(c2) and c2['total']==full['total'],'second_page_mismatch')
                require(f2['has_more']==(full['total']>200),'second_has_more')
            if full['items']:first_entry=first_entry or full['items'][0]['id']
            timings=[]
            for _ in range(10):
                repeated,tm=call('ctf_search_pinned',query=q,limit=100,offset=0)
                require(ids(repeated)==ids(fast),'repeat_page_mismatch');timings.append(tm)
            timings.sort();metrics.append({'query_number':i+1,'total_count':full['total'],'samples':len(timings),'warm_median_ms':round((timings[4]+timings[5])/2,2),'warm_p95_ms':round(timings[math.ceil(.95*len(timings))-1],2)})
        report['queries']=metrics;report['checks']['counted_fast_pagination_and_repeated_source_checks']='PASS'
        # Entire selected result, not merely the UI page; compare all 15 fields and JSON.
        q='ext:mkv';rows=None
        for fmt in ['csv','parquet','json']:
            r,_=call('ctf_export_pinned',format=fmt,output=str(work/('selected.'+fmt)),mode='search',query=q)
            if rows is None:rows=r['rows']
            require(r['rows']==rows,'export_count_mismatch')
        import duckdb
        con=duckdb.connect(config={'threads':'1','memory_limit':'128MB'})
        parq=con.execute('SELECT * FROM read_parquet(?)',[str(work/'selected.parquet')])
        fields=[x[0] for x in parq.description]
        with (work/'selected.csv').open(encoding='utf-8-sig',newline='') as stream:
            csv_rows=list(csv.DictReader(stream))
        json_rows=json.loads((work/'selected.json').read_text())['entries']
        parquet_rows=parq.fetchall();con.close()
        require(len(fields)==15,'export_schema_column_count')
        require(len(csv_rows)==len(json_rows)==len(parquet_rows)==rows,'independent_count_mismatch')
        for c,j,p in zip(csv_rows,json_rows,parquet_rows):
            require([c[n] for n in fields]==['' if x is None else str(x) for x in p],'csv_parquet_values')
            require([j.get(n) for n in fields]==list(p),'json_parquet_values')
        report['checks']['selected_csv_parquet_json_all_15_fields']='PASS';report['selected_export_rows']=rows
        exported_hash=sha(work/'selected.csv')
        call('ctf_export_pinned','file_exists',format='csv',output=str(work/'selected.csv'),mode='search',query=q)
        require(sha(work/'selected.csv')==exported_hash,'overwrite_changed_output')
        call('ctf_search_pinned','source_changed',generation='0'*64,query='ext:mp3')
        call('ctf_duplicates_pinned','unsupported',duplicate_mode='hash')
        report['checks']['stale_generation_no_clobber_and_no_hash_claim']='PASS'
        d,_=call('ctf_duplicates_pinned',query='ext:mp3',duplicate_mode='name_size',limit=100)
        require(d['verified'] is False and d['basis']=='metadata_candidates','false_duplicate_claim')
        export,_=call('ctf_export_pinned',format='json',output=str(work/'duplicates.json'),mode='duplicates',query='ext:mp3',duplicate_mode='name_size')
        dr=json.loads((work/'duplicates.json').read_text())['entries']
        # Export is volume_id/id ordered; native page is group/path ordered.
        conn=sqlite3.connect(mirror.as_uri()+'?mode=ro',uri=True)
        conn.create_function('unicode_lower',1,lambda text: text.lower(),deterministic=True)
        selected=conn.execute("SELECT id,volume_id,relative_path,name_norm,size FROM entries WHERE kind='file' AND ext='mp3'").fetchall()
        from collections import Counter
        counts=Counter((r[3],r[4]) for r in selected)
        expected={r[0]:(counts[(r[3],r[4])],str(r[3])+':'+str(r[4])) for r in selected if counts[(r[3],r[4])]>1}
        actual={r['id']:(r['duplicate_count'],r['duplicate_group']) for r in dr}
        require(len(actual)==len(dr) and actual==expected,'duplicate_export_membership_mismatch')
        require([(r['volume_id'],r['id']) for r in dr]==sorted((r['volume_id'],r['id']) for r in dr),'duplicate_export_order')
        ascii_lower=str.maketrans('ABCDEFGHIJKLMNOPQRSTUVWXYZ','abcdefghijklmnopqrstuvwxyz')
        ordered=sorted((r for r in selected if r[0] in expected),key=lambda r:(expected[r[0]][1].translate(ascii_lower),r[1],r[2],r[0]))
        require([r[0] for r in ordered[:100]]==[r['id'] for r in d['items']],'duplicate_native_order')
        require(d['has_more']==(len(expected)>100),'duplicate_has_more');conn.close()
        require(len(dr)==export['rows'] and all(x['duplicate_count']>1 for x in dr),'duplicate_membership')
        report['checks']['metadata_duplicates_complete_export']='PASS';report['duplicate_export_rows']=len(dr)
        require(first_entry is not None,'empty_catalog_for_annotation_test')
        nbase={'annotation_db':str(notes)}
        call('ctf_annotations_set_pinned',entry_id=first_entry,note='Acceptance test note',tags='qa',expected_revision=0,**nbase)
        call('ctf_annotations_set_pinned','annotation_revision_conflict',entry_id=first_entry,note='Do not replace',tags='qa',expected_revision=0,**nbase)
        listed,_=call('ctf_annotations_list_pinned',**nbase);require(listed['total']==1 and listed['items'][0]['revision']==1,'annotation_persistence')
        doc=listed['document_id'];backup=work/'test.wia.backup.sqlite'
        snap,_=call('ctf_annotations_backup_pinned',backup_path=str(backup),expected_document_id=doc,backup='user_approved_new_snapshot',**nbase)
        call('ctf_annotations_set_pinned',entry_id=first_entry,note='Changed test note',tags='qa',expected_revision=1,**nbase)
        restored=work/'restored.wia.sqlite'
        call('ctf_annotations_restore_pinned',annotation_db=str(restored),backup_path=str(backup),expected_document_id=doc,expected_backup_sha256=snap['backup_sha256'],restore='user_approved_new_sidecar')
        check,_=call('ctf_annotations_list_pinned',annotation_db=str(restored));require(check['items'][0]['note']=='Acceptance test note','snapshot_restore')
        current,_=call('ctf_annotations_list_pinned',**nbase);require(current['items'][0]['note']=='Changed test note','restore_overwrote_live')
        require(sha(mirror)==mirror_before,'derived_mirror_modified')
        report['checks']['independent_annotations_cas_backup_restore']='PASS'
        report['checks']['derived_mirror_unchanged']='PASS';report['status']='passed_available_checks'
    except Exception as exc:
        message=str(exc)
        report['error_code']=message if message.replace('_','').isalnum() else type(exc).__name__
    finally:
        client.close();report['source_unchanged']=sha(source)==before
        if not report['source_unchanged']:report['status']='failed';report['error_code']='source_changed'
    return report


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--cli',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);ap.add_argument('source',type=Path);args=ap.parse_args()
    if args.output.exists():raise SystemExit('Report exists; refusing overwrite')
    with tempfile.TemporaryDirectory(prefix='whereisit-final-qa-') as td:
        result=run(args.cli.resolve(),args.source.resolve(),Path(td))
    with args.output.open('x',encoding='utf-8') as stream:
        json.dump(result,stream,indent=2);stream.write('\n')
    print(json.dumps({'status':result['status'],'checks':result['checks'],'entries':result.get('entries'),'error_code':result.get('error_code')}))
    raise SystemExit(0 if result['status']=='passed_available_checks' else 1)
