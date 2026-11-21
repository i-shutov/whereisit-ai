# Synthetic original-application CTF regression

The Windows x64 artifact from Actions run 44891158178 (commit `40c6b695958e49cbdad78affa314b139dd28d61b`) failed this check on Windows 11 build 22631. The CLI exported 60 disks and 180 entries, and WhereIsIt 2014 build 220 reported `Can't open catalog file`. An independent vendor-created control opened in the same application.

1. Generate 60 local synthetic volumes with `tools/qa/windows10-egui/Invoke-WhereIsItEguiQA.ps1 -PreviewDir <verified-preview> -EvidenceRoot <new-private-directory> -DiskCount 60 -SkipGui` without `-CategoryChecks`. Use the generated `synthetic-catalog.sqlite` and a new output path.
2. Invoke the verified CLI `export_ctf` service operation with that database and a new `.CTF` output. Do not set `allow_lossy`.
3. In installed WhereIsIt 2014 build 220, use Open and select the newly generated CTF. Expected: the catalog opens and all 60 synthetic disks and entries can be browsed. Failure or an error dialog is FAIL; no vendor application is BLOCKED. Record only synthetic screenshots locally.
4. Positive control: use an independently created, explicitly authorized vendor CTF copy, located in a comparable local directory, to confirm the vendor application can open a catalog. Do not publish the control or its identifying data.

The writer had declared a fixed 184-byte catalog offset table, the correct value for 45 blocks. The table contains one four-byte offset per block and a four-byte sentinel. A 60-block catalog requires 244 bytes. The writer now derives the length from the block count. The parser recognizes the previous fixed declaration for compatibility and still validates the actual table and sentinel.

## Verified result

**PASS for synthetic catalogs.** WhereIsIt 2014 build 220 opened a corrected 60-disk minimal catalog and displayed its sample file. It also opened the corrected CLI export of 60 disks and 180 entries. Disk 060 displayed the Cyrillic folder `Кино` and `clip-060.mkv`. After `update_entry` changed the file description in a copy of the synthetic database, the original application opened a fresh export and displayed the exact updated description.

Separate authorized private-copy checks also passed in both description-edit directions. Identifying data and raw evidence remain private; see [sanitized acceptance results](WIN11_CTF_PATH_ACCEPTANCE_REDACTED_20261121.md). This does not establish compatibility with every catalog variant or vendor version.
