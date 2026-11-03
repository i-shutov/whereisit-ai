# UX44 — private native global-search latency session

## Behavior
The native Windows Win32 and macOS SwiftUI search clients accumulate timings for up to 128 freshly accepted global search results pages; no raw query/film/file/source path, volume identifier, CTF SHA or timing telemetry is persisted, copied or published. Search timing includes native dispatch and source-generation SHA authorization. A source failure, stopped/old response, selected disk, browsing, metadata duplicates or candidate copies do not contribute.

View menu (Win): **Global-search latency**, Ctrl+Shift+8; **Reset**, Ctrl+Shift+7. Catalog → Local search latency (macOS): **Show**, Command+Shift+8; **Reset**, Command+Shift+7. This deliberately adds no toolbar buttons. Samples clear on catalog initialization and successful linked CTF sync. Fewer than 30 samples: show progress only; 30–128 samples: nearest-rank p50 and p95 of the bounded session. Display always explicitly states NOT hardware/release acceptance.

## Regression acceptance
Windows compiled Rust test exercises exclusion of unverified/single-disk/duplicate/candidate/empty queries; nearest-rank p50 20 and p95 38 from 1..40; 128 ring bound; reset. Cross-native Python contract asserts both UI commands, bounded storage and generation gating. Do not skip existing linked CTF mutation/restored-mtime, read-only original, untrusted UUID and installed upgrade tests. Require **exact-HEAD** native Linux, Windows x64, macOS ARM64/Intel, offline, integration and RC2→RC3 workflows.

## Owner-only reproducibility
Reset the session on permitted real hardware, warm source cache, and issue 30+ representative GLOBAL film filename / extension / video-expression queries, browsing multiple verified pages. Use the local menu to read the aggregate; record only sanitized sample count/hardware/cache state/p50/p95 privately. Repeat on Win10/11 and Mac Intel/ARM64 and compare. Confirm failures, cancellations and individual-disk search cannot pollute samples. Check keyboard, VoiceOver and narrow 880px/1366px layouts. **This diagnostic does not prove representative real p95 <=150ms** and never validates physical file presence, disk identity or content equality. Stable 1.0 also requires original-modified and historic CTF, actual GUI/USB approval, signing and notarization. Keep RC3 unsigned prerelease and #57 draft.
