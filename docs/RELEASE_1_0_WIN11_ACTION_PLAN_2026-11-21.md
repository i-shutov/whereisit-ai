# WhereIsIt AI — Windows1.0 release

**Owner decision22.11.2026:** publish Windows x64 stable1.0 unsigned after installer acceptance. USB physical pairing/bound-device rescan disabled; ordinary metadata scan retained. Tabs/workspaces, sunburst/archive slices and local analyzer deferred. macOS/Linux products later; portable code and useful cross-platform checks remain.

## Accepted base

RC4/main751fa545 is immutable and published. Prior native installed Win11 evidence covers60 generated volumes, full44-volume/3046133-entry CTF, guarded copy edit/new export independently accepted by original WhereIsIt2014build220, reverse vendor edit/import, unchanged roundtrip, full15-field CSV/Parquet parity, categories/notes/navigation/exports, cancellation and external-data-preserving lifecycle. Source catalogs remain read-only.

Windows10 was tested jointly with the owner earlier; its removed machine need not be recreated. Do not claim an exact new1.0 Windows10 run. The owner plans later personal testing.

## 1.0 changes and acceptance

- Actual1.0.0 version/lock/notices/Windows metadata, honest unsigned label; no RC rename/retag.
- USB pair/unpair/bound-rescan rejects before opening files or probinghardware; no such controls in diskproperties. Deferred implementation retained forfuture. Ordinary metadata scan remains.
- Windows stable publisher selects exactly standardegui setup+ACCEPTANCE+provenance+SHA256SUMS. Require owner unsigned approval, exact currentmain/source/run, passingWindows/test jobs, cleanacceptance and checksum. Draft/downloadverify precedes stable/latest publication. ExistingRC3/RC4 unchanged.
- Publication only after actualexactinstaller GUI. Manualworkflow or explicit `release/windows-1.0-publish` controlbranch may start it; rootcreatescontrolbranch pointingexactvalidatedmain only afterGUI. Noautomaticstablepublication onmainmerge.
- macOS Rust/Swift compile and tests remain; macOS packages/notarization are not Windows1.0 requirements. Historical upgrade probes stayfrozen; newWindowsRC4→1.0 upgrade/uninstall/rollback checks generatedexternalcatalog/notes/tags/categories preservation.

Localvalidation:301RusttestsPASS, strictallfeatureClippyPASS,15RC+stablepackagingtestsPASS,2identitytestsPASS,9upgradeversiontestsPASS. Installed1.0GUI/hostedfinalSHA acceptance is recorded only after observation. `stable_ready=false` during preflight; enable after in-scopeinstalledQA, then gatepublication onexacthostedartifact acceptance.

## Support boundaries

CTF: observed uncompressedCatalog3.00 subset, opaqueunsupportedstructures failclosed. No silentloss/overwrite, no sourcewrite, no contenthashing/thumbnails/executedcatalogedfiles/defaultnetwork/cleanup. Metadata matches are not proven backups or byte equality.

WarmWin11SLO excludes concurrentUIautomation and GUIrendering: lastRC4idlemaxp9560.47ms; separateconcurrent237.38msFAILretained. No cold-cache or arbitraryload guarantee. Repeataffected1.0workloads withoutconcurrentUIautomation.

P1 defects and optionalfeatures are distinct. Deferredtabs/sunburst/analyzer are approvedfuture scope, not unimplementedmandatory1.0bugs. PR58remainsUXauthority. Finalsignoff records exactinstalledGUI/independentoriginalapp/data evidence and releaseverdict; a newly hiredexternalQA person is not required.

[Gate matrix](../release/RELEASE_1_0_GATE_MATRIX_20261116.json), [acceptance](../release/ACCEPTANCE.json), [1.0release notes](releases/v1.0.0.md). Publicevidence is aggregate/synthetic only; no privatecatalogs/paths/digests/licenses/screenshots.
