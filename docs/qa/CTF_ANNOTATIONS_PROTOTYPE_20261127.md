# CTF annotations: first PR58 prototype checkpoint — 2026-11-27

Scope: synthetic React/Express design reference only. No real CTF, SQLite
annotation service, native egui implementation or release readiness is claimed.
Plan commit `3eb185f` was pushed before implementation began.

## Implemented

The existing docked inspector now includes localized RU/EN note, tag and category
fields, draft/saved/error states, and an explicit demo-only external-description
change action. The PR58 disk/tree/table composition remains intact.

Synthetic browser storage uses an explicit mock document identity plus volume
and entry addresses. Saving a local edit preserves its source baseline. External
changes use a field-level three-way comparison; accepting an external description
does not discard unrelated local tags/categories. Unambiguous changes advance
the synthetic source baseline. Malformed saved records are rejected and their
controls cannot overwrite the unreadable record. In-memory drafts remain while
switching selection, including after a browser-storage write failure.

The mock-copy identity is covered by utility tests; no second GUI catalog/copy
workflow is claimed. These synthetic addresses do not define native identity.

## Actual browser interaction

On localhost, the existing disk overview was opened, followed by a folder and
its generated file. The inspector saved a Unicode note, tags and a category.
After browser reload and reselecting the same generated file, all three values
were restored. A simulated external change showed both descriptions and disabled
ordinary Save until resolution. Reload restored the unresolved conflict.

Choosing the external description retained the local tags and category. A later
local edit/save followed by another external change still produced a conflict;
choosing the local value preserved it. Screenshots were visually inspected;
no private catalog or screenshot is committed. GUI storage-failure injection,
native persistence, real external saves and CTF exports were not tested here.

Validation after integration: `npm run lint`, `npm run test:workspaces` (28/28)
and `npm run build` passed. The conflict status badge was also checked in the
browser after the final change; it reports that resolution is required.

## Backend handoff

The existing `ctf_annotations.rs` / `ctf_annotation_backup.rs` store and tests
already provide durable notes/tags, revisions, review/activation and backups.
They currently require the pinned SQLite mirror. Direct preview refuses writes
and has a different per-open generation token. Reusing that token as persisted
annotation identity would lose restart continuity.

[The draft contract](../CTF_DIRECT_ANNOTATIONS_CONTRACT_20261127.md) keeps the
session token for stale-request rejection and adds a separate durable source
generation/document identity boundary. It preserves `.wia.sqlite` ownership,
requires explicit schema/backup decisions for categories and ABA-safe retries,
and prohibits a hidden full import. This contract is not yet a registered API.

Next backend patch: reuse the existing store with direct-snapshot entry
validation and same-source reopen tests; keep native writes disabled until
the durable identity and revision contract is implemented. Clarify the mapping
between existing native `note` and proposed CTF-description overrides before
porting this UX; do not silently reinterpret stored annotations.
