# PR #58 egui defects and follow-up

| ID | Priority | Finding / impact | State and evidence |
|---|---|---|---|
| UX-01 | P0 | Inspector had no action to navigate from a file to its containing catalog folder; Back discarded the selected result. | FIXED for returned page: parent-id route, Back restores scope/offset/selected record. Unit test and actual egui search → folder → Back with synthetic file passed. Large-folder selected-file pagination and scroll restoration remain OPEN. |
| UX-02 | P0 | “Candidates on other disks” called `copy_candidates` without `other_disks_only`, so it could return the selected file on the same disk. | FIXED in request before pagination; regression checks request; service and GUI verified #61 → #62 and Back on two synthetic volumes. |
| QA-01 | P1 | WGC frame timeout and desktop crops exposed peripheral windows. | PNG capture FIXED for opt-in egui QA build through renderer screenshot callback; mouse geometry still BLOCKED by WGC automation. |
| UX-03 | P1 | Sidebar uses two lines per disk, lacks sorting and free-space visualization. | OPEN; compare with PR58 without inventing disk capacity. |
| UX-04 | P1 | Archive slices, sunburst, local analyzer and category UI are absent. | OPEN; service-backed vertical slices required; prototype analytics are simulated. |
| UX-05 | P0 | Containing-folder navigation lost the selected file when it was beyond the first page. Back scroll position was not restored. | FIXED: snapshot-consistent service rank/page and selected-row reveal; GUI verified item 270 and Back. Back scroll restoration is implemented and unit checked, but actual scrolled-position GUI verification remains OPEN. |
| QA-02 | P1 | No paired PR58/egui visual atlas, complete mouse/timing or DPI/theme comparison. | PARTIAL: browser-rendered reference images and several native pairs are now captured. Progress, cancel, error, export/no-clobber pairs and pointer/DPI coverage remain open. |
| UX-06 | P1 | Language and theme revert to Russian and dark on startup. | FIXED locally: eframe stores only language and theme; actual Windows GUI keyboard selection of English/light survived a normal close and restart with isolated settings. Invalid values fall back to Russian/dark. |
| UX-07 | P1 | A previously computed status message stays in its old language after switching language. | FIXED: retain both localized status strings and select at rendering. GUI RU → EN → RU translated the existing row count without re-running the search; regression also covers export count and path. Service errors and other UI strings still require a systematic bilingual audit. |

Separate future research: video-header plugin, sparse/full hash match levels and asserted throughput have no validated real implementation in this audit; do not present the prototype's numbers as measured. Stable 1.0 remains NO-GO.
