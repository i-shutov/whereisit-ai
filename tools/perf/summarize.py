#!/usr/bin/env python3
import csv,json,sys
from pathlib import Path
root=Path(sys.argv[1]);lines=['# CTF RAM / SQLite / columnar measured study','',
'All data below are synthetic. No personal catalog was uploaded. Times are milliseconds; medians of six warm repetitions after one first run. First run is not a cold-OS-cache measurement. The released application returns hydrated Entry objects; the other comparisons return ordered IDs only.','']
for d in sorted(root.iterdir()):
    if not d.is_dir() or not (d/'ram.json').exists(): continue
    reports={p.stem:json.loads(p.read_text()) for p in d.glob('*.json')}
    ram=reports['ram'];gen=reports.get('generate',{});prep=reports.get('columnar-prepare',{})
    lines += [f'## {ram["rows"]:,} entries', '', f'Synthetic CTF: {gen.get("ctf_bytes")} bytes. Unchanged roundtrip equal: {ram["unchanged_ctf_equal"]}.',
              f'CTF read / parse / arena construction / ordered indexes: {ram["read_s"]:.3f} / {ram["parse_s"]:.3f} / {ram["arena_build_s"]:.3f} / {ram["ordered_index_build_s"]:.3f} seconds.',
              f'Hot row size: {ram["fixed_hot_row_bytes"]} bytes; major arena capacities: {ram["arena_major_capacity_bytes"]} bytes, excluding raw CTF and associative containers. Process high-water RSS is separately recorded and includes parsing/serialization transient allocations.', '']
    engines=[x for x in ['app','sqlite-direct','sqlite-tuned','sqlite-ram-tuned','duck-file','duck-ram','duck-parquet','duck-sorted','polars-ram','polars-parquet'] if x in reports]
    header=['Case','RAM scan','RAM candidate lists']+engines
    lines += ['| '+' | '.join(header)+' |','|'+'---|'*len(header)]
    rows=[]
    for q,r in enumerate(ram['queries']):
        row={'id':q,'query':r['query'],'RAM_scan_ms':r['ram_scan_page']['median_ms'],'RAM_candidates_ms':r['ram_ordered_candidates_page']['median_ms'],'matches':r['total']}
        cells=[f'Q{q}: {r["query"]}',f'{row["RAM_scan_ms"]:.3f}',f'{row["RAM_candidates_ms"]:.3f}']
        for engine in engines:
            val=reports[engine]['queries'][q]['page']['median_ms'];row[engine+'_ms']=val;cells.append(f'{val:.3f}')
        lines.append('| '+' | '.join(cells)+' |');rows.append(row)
    with (d/'timings.csv').open('w',newline='',encoding='utf-8') as f:
        w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)
    lines += ['', '### Storage / startup / duplicate grouping', '',
              '| Engine | File bytes | Load/copy seconds | Peak RSS KiB | Duplicate build ms |','|---|---:|---:|---:|---:|',
              f'| Rust arena process | original CTF retained | {ram["read_s"]+ram["parse_s"]+ram["arena_build_s"]+ram["ordered_index_build_s"]:.3f} | {ram["memory_after_queries"].get("peak_rss_kib")} | {ram["duplicate_build_s"]*1000:.3f} |']
    for engine in engines:
        r=reports[engine];load=r.get('open_and_load_s',r.get('copy_to_ram_s',r.get('import_s')));b=r.get('sqlite_bytes','');peak=r.get('memory',{}).get('peak_rss_kib');dup=r.get('duplicates',{}).get('first_build_ms','')
        lines.append(f'| {engine} | {b} | {load} | {peak} | {dup} |')
    lines += ['',f'DuckDB + Parquet preparation: `{json.dumps(prep,ensure_ascii=False)}`','',
              '**Coverage:** all compared query first-page ID sequences and exact match counts are asserted against the Rust reference. Duplicate group and member counts are also asserted. This does not prove all possible expressions, payload types or all full result sequences. CTF external-application acceptance is not tested.','']
(root/'SUMMARY.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print('\n'.join(lines))
