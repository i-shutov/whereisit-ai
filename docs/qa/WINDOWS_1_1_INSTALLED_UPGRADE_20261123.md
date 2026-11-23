# Windows 1.1 installer and 1.0 upgrade protocol

PR #88 was merged as `a401abb2ce1598667ccc63f570e05fa4895a9562`, then #89 was retargeted to main and merged as `ba715be3ac50749d2191734f5c4895d7a5716261`. The #88 merge preserved ancestry and its source tree matched the previous #89 base. The retargeted #89 diff retained exactly the 30 analyzer/prototype/dependency/docs files. Its unchanged exact head passed 16 checks / 8 workflows before the merge; no reviews or inline comments were outstanding.

## Exact candidate

Version `1.1.0-rc.1` is an unsigned Windows-x64 installer candidate. The released `v1.0.0` tag/assets remain immutable. Its acceptance is preserved in `release/history/v1.0.0.json`; the new candidate has separate acceptance, stable_ready=false and candidate_publication_approved=false. macOS/Linux releases remain deferred while portable builds/tests continue. No automatic tag or release is authorized by this protocol.

The candidate must come from a successful exact-main native CI run. Verify artifact ZIP digest, provenance commit/run/version, installer checksum and installed GUI/CLI hashes. PR artifacts are development checks and do not substitute for that main artifact.

## Local Windows GUI sequence

1. Verify the published 1.0.0 installer and its exact-source CI payload manifest. Confirm the installed baseline version and executable hashes. Use the existing product AppId and installation directory for a real in-place upgrade.
2. Generate dedicated external SQLite, Unicode notes/tags/category assignments, a separate user-store and source files outside the repository. Record metadata, logical rows, hashes, SQLite integrity and old-version GUI observations. Never select the original user CTF.
3. Interact with the candidate installer in the Administrator RDP desktop. Record install/upgrade completion and the version in the installed application. Launch the installed payload, not a preview.
4. Verify external catalog/user-store preservation, Unicode note/category visibility, search and workspace context, slices/sunburst and the explicit local analyzer. Restart and recheck persistence. Exercise review CSV without overwriting an existing target.
5. Verify uninstall/rollback/reinstall against the disposable external data where required; finish with the candidate installed. Hash and query checks supplement the actual GUI actions.

The hosted exact-source upgrade smoke separately covers pinned source 1.0.0 → 1.1.0-rc.1, generated catalog/category/note/tag/search parity, uninstall and rollback. Its report explicitly says gui_observed=false. It does not certify the published 1.0 bytes or this physical Windows desktop by itself.

This source file is the pre-build protocol. Actual post-build PASS/FAIL evidence is tied to the tested main SHA and installer in the candidate PR's final validation record and the owner's private local evidence. Committing a test plan is not a GUI PASS. Raw paths, catalogs, user-store files and screenshots stay local; only sanitized results are public.
