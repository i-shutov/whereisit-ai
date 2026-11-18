# PR #58 egui defects and follow-up

| ID | Priority | Finding / impact | State and evidence |
|---|---|---|---|
| UX-01 | P0 | Inspector had no action to navigate from a file to its containing catalog folder; Back discarded the selected result. | FIXED for returned page: parent-id route, Back restores scope/offset/selected record. Unit test and actual egui search → folder → Back with synthetic file passed. Large-folder selected-file pagination and scroll restoration remain OPEN. |
| UX-02 | P0 | “Candidates on other disks” called `copy_candidates` without `other_disks_only`, so it could return the selected file on the same disk. | FIXED in request before pagination; regression checks request; service verified two synthetic volumes returns the other volume only. Repeat GUI scenario on final binary OPEN. |
| QA-01 | P1 | WGC frame timeout and desktop crops exposed peripheral windows. | PNG capture FIXED for opt-in egui QA build through renderer screenshot callback; mouse geometry still BLOCKED by WGC automation. |
| UX-03 | P1 | Sidebar uses two lines per disk, lacks sorting and free-space visualization. | OPEN; compare with PR58 without inventing disk capacity. |
| UX-04 | P1 | Archive slices, sunburst, local analyzer and category UI are absent. | OPEN; service-backed vertical slices required; prototype analytics are simulated. |
| UX-05 | P1 | Back and containing-folder focus are incomplete for records beyond first folder page; scroll restoration is not verified. | OPEN; avoid claiming complete P0 parity. |
| QA-02 | P1 | No paired PR58/egui visual atlas, complete mouse/timing or DPI/theme comparison. | OPEN; PR58 runtime dependencies unavailable during this block, system DPI unchanged. |

Separate future research: video-header plugin, sparse/full hash match levels and asserted throughput have no validated real implementation in this audit; do not present the prototype's numbers as measured. Stable 1.0 remains NO-GO.
