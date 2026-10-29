#!/usr/bin/env python3
import json,sys
from pathlib import Path
root=Path(sys.argv[1]);lines=['# Indexed RAM and full-Entry SQLite A/B','', 'Synthetic-only evidence. Six warm samples after a first run; no cold-cache or p95 claim. RAM computes the complete result bitmap, exact count, and first 250 IDs. SQLite computes a hydrated page of 250 complete Entry values with a continuation probe. These are deliberately distinct workloads; the SQL A/B variants themselves have identical full Entry response shape.','']
for d in sorted(root.iterdir()):
    if not d.is_dir() or not (d/'indexed-ram.json').exists():continue
    r=json.loads((d/'indexed-ram.json').read_text());sql=json.loads((d/'native-sql-ab.json').read_text()) if (d/'native-sql-ab.json').exists() else None
    lines += [f'## {d.name} — {r["rows"]:,} records','',f'Names/folder paths/descriptions distinct: {r["unique_names"]:,} / {r["unique_folder_paths"]:,} / {r["unique_descriptions"]:,}.',
              f'Canonical ingestion/interning: {r["canonical_read_and_intern_s"]:.3f} s. Index construction: {r["all_index_build_s"]:.3f} s. Integer-key duplicate grouping: {r["integer_key_duplicate_build_s"]:.3f} s.',
              f'RSS after build / peak (KiB): {r["memory_after_build"]}. This process loads canonical data, not the raw CTF; startup is NOT directly comparable to the direct-CTF arena test.','',
              '| Case | RAM full set + count + page, ms | SQL original full page, ms | SQL index only, ms | SQL index + fixed predicates, ms |',
              '|---|---:|---:|---:|---:|']
    for q in r['queries']:
        v=sql['queries'][q['id']] if sql else {}
        def time(key):
            x=v.get(key,{}).get('median_ms');return '-' if x is None else f'{x:.3f}'
        lines.append(f'| Q{q["id"]}: {q["query"]} | {q["median_ms"]:.3f} | {time("original")} | {time("covering_index_only")} | {time("equivalent_fixed_predicate")} |')
    lines += ['', 'Native SQL full pages, including all fields, are checked against the released core. RAM first-page IDs and exact counts are checked against the reference; the report also records a SHA-256 digest of all ordered matching IDs for future exhaustive parity. Fixed-predicate category optimization assumes the known synthetic category assignment; it is not a claim that arbitrary categories have been solved.', '']
(root/'SUMMARY.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print('\n'.join(lines))
