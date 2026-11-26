# P1-ANALYTICS-TEXT-HIGHLIGHT

24 November 2026. Status: **OPEN in published 1.1.0; repaired in development**,
not a new release acceptance.

The direct-CTF analytics equivalence test found that the SQLite reference itself
fails for a valid highlight expression containing `description:`. Both
`matched_totals` and `matched_group_totals` compile the normal shared query
language, then execute its predicate against `entries_core`. That compact table
does not contain `description`, `tags` or `search_text`; those fields are provided
by the metadata view. The observed error is `no such column: description`.
Default-text and tag predicates use the same incompatible boundary.

The affected SQL and query-field mappings are also present in the published
1.1.0 source commit `7f684f47aab42fb211dfa0af1085ace311ef9f8d`. This is an existing
analytics-query defect exposed by the new parity cases, rather than a CTF parse
or source-data mutation. The catalog contents are not changed by the failed
read-only request.

Repair the highlighted-counter queries to read the metadata fields required by
the compiled predicate while preserving scope, category filtering, bound
parameters and matched-group totals. Keep the cheap non-highlight aggregates on
their compact path. Add synthetic runtime cases for descriptions, tags, default
text, Boolean expressions, folders and grouped counters. Retain the independent
direct-CTF versus SQLite complete-response comparison; do not remove its failing
query or change expected values to conceal the error.

Closure requires passing runtime regressions, CI and the affected native GUI
scenario on the candidate. The published installer is unchanged and
`stable_ready=false`. No private catalog, source digest, path, row or screenshot
is included in this report.

The development fix selects the metadata-capable query table when the predicate
needs it. Synthetic runtime regressions passed. Native GUI interaction with a
generated SQLite catalog confirmed correct description, tag and default-text
highlight counts and bytes; see [the checkpoint](DIRECT_CTF_ANALYTICS_20261124.md).
Exact candidate CI and installed-package acceptance remain pending.
