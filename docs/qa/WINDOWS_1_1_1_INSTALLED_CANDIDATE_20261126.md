# Windows 1.1.1 installed candidate — 26 November 2026

**Historical candidate checkpoint.** Later reverse-CTF acceptance and release
readiness are recorded in [the stable readiness report](WINDOWS_1_1_1_STABLE_ACCEPTANCE_20261126.md).
The pending gates below describe this earlier checkpoint; they are not a
replacement for the later exact-artifact publication checks.

**Overall candidate gate: PARTIAL.** The local installed upgrade and the exact GitHub Actions unsigned Windows x64 installer passed the GUI checks recorded below. The exercised original-WhereIsIt path establishes forward compatibility for the tested modified export of a full catalog copy only. It does not establish release readiness or universal CTF compatibility. Keep `release/ACCEPTANCE.json` at `stable_ready=false`.

## Candidate identity and scope

- Version: 1.1.1, Windows x64, unsigned.
- Candidate source commit: `3cf2d36fe2929858cd768ef2d080a82d1725ca5a`.
- The local installer and the downloaded GitHub Actions standard product installer used standard product binaries, without QA-only features.
- Testing took place in an interactive Windows Administrator RDP session. This records the observed GUI session and is not CI or a hosted-runner result.

The exact GitHub Actions product installer was downloaded from artifact `15143743307` in run `45873945126`. The archive, installer checksum, and provenance were verified. Both installed executables matched the hosted provenance. This run used candidate source commit `3cf2d36fe2929858cd768ef2d080a82d1725ca5a`.

## Installed upgrade and GUI — PASS for this local candidate

The 1.1.0 installation was upgraded to 1.1.1 through the installer wizard. Both installed executables matched the candidate binaries. The native SQLite catalog contained five entries; search found the synthetic note, the Unicode category was visible, and the disk label and shelf preferences were visible. A normal close and restart preserved the observed state.

A protected CTF copy containing 44 volumes and 3,046,133 records was opened directly in the GUI. External query pages 1–250 and 251–500 were exercised. The GUI was closed normally before uninstall. The original source remained protected; this report records no source paths or hashes.

The exact GitHub Actions installer was also installed in the GUI. Its 1.1.1 GUI opened the protected full CTF and passed query pages 1–250 and 251–500. The native catalog was reopened after restart; its one disk and five entries were present, and the disk label and shelf preferences remained visible. The application closed normally. The hosted installed CLI retained notes, tags, category, and preference semantics. The four external fixtures and the existing default user store remained byte-for-byte unchanged.

Uninstall completed successfully. These generated or user-state files remained byte-for-byte unchanged through uninstall: the native SQLite catalog, a secondary SQLite catalog, the application-format preferences store, an invalid CTF placeholder, and the pre-existing default user store. The GUI was then installed again. Notes, tags, categories, preferences, and CLI read semantics remained intact, and both installed binaries still matched the candidate binaries.

The invalid CTF placeholder verifies preservation of an external file only. It is not CTF parsing, round-trip, or compatibility evidence.

## Original WhereIsIt forward compatibility — PASS for this modified export

The locally installed 1.1.1 CLI used a verified 1.1.0 full SQLite catalog copied into the private QA area, updated one folder description to the full synthetic note `QA111 compatibility note — 東京 Ω`, and exported to a new CTF without overwrite or lossy-export warnings. Export took 16.18 seconds. Original WhereIsIt version 2014.0.0.220 opened that exact exported CTF by command-line argument. In its GUI, the selected folder and Description Viewer displayed the complete synthetic note. The application was closed normally without saving. The original/master input and full protected source copy remained unchanged by hash.

This is forward-compatibility evidence for the tested modified export of a full catalog copy; only the inserted note was synthetic. Reverse compatibility (original WhereIsIt writing a CTF for 1.1.1 to import) and all-field comparison are `NOT RUN`; no universal compatibility claim is made.

## Installed analytics highlight regression — PASS

The exact hosted installer GUI opened the generated native SQLite fixture and
applied description, tag and default-text highlight queries in the volume map.
Each query selected the expected one file and 11 bytes from three files and
33 bytes. The chart updated and no missing-column SQL error appeared. This
checks the installed candidate repair of P1-ANALYTICS-TEXT-HIGHLIGHT; it does
not change the already published 1.1.0 installer.

## Remaining gates

- PR 101's required `test`, `windows`, and exact upgrade aggregate job (run `45873945384`) succeeded. The current CI snapshot also had 13 passed jobs, 2 macOS jobs nonterminal, and 1 publication job skipped. This is not a claim that every workflow job passed.
- The exact GitHub Actions artifact was installed and its GUI checked as described above; this was not a repeat of the local 1.1.0 → 1.1.1 wizard lifecycle. The separate exact-source hosted 1.1.0 → 1.1.1 upgrade CI passed.
- Reverse original-WhereIsIt compatibility and semantic all-field comparison are `NOT RUN`.
- Earlier code-level performance measurements are recorded separately; this installed-candidate check is not a performance benchmark and does not claim those results for this installed build.
- Stable publication remains blocked pending the remaining acceptance and explicit readiness decision. Keep `stable_ready=false`; preserve existing tags and assets as immutable.

No private catalog names, local paths, source-file hashes, screenshots, or software license files are included in this report.
