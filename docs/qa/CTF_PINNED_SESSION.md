# CTF pinned read-only search — session-aware performance mode

This is an opt-in performance mode for a **resident native GUI/FFI service**
or one long-lived atlas serve process. It is NOT identical to strict per-call
verification against malicious local disk cache tampering.

## Two distinct contracts

- Strict ctf_sync rehashes the original CTF and full derived immutable SQLite
  file, validates their manifest/recipe and registers a bounded process-local
  generation pin. It returns a 64-character generation digest.
- ctf_search_pinned requires exactly that generation in the same process.
  It validates the query before hashing any file, reads the original CTF
  digest before and after every search (including same-size/restored-mtime
  edits), compares derived SQLite file identity before and after, and uses a
  read-only connection. It fully rehashes the SQLite cache at least every
  60 seconds of pin life on the next query. The response explicitly states
  the verification mode and whether full rehash occurred.

The existing ctf_search remains strict on EVERY call, including full
verification of the derived DB. The pinned mode assumes a trusted local cache
directory: a malicious local writer capable of replacing database content
while spoofing exact size, timestamps and filesystem identity may evade the
lightweight checks until the next full DB hash. This is a known tradeoff, not
a cryptographic guarantee. Use strict mode for stronger assurance.

## Protocol (one process)

Start atlas serve or use an in-process native GUI bridge.

First send JSON:
    {"op":"ctf_sync","path":"/private/catalog.CTF","cache_root":"/private/cache"}

Then send:
    {"op":"ctf_search_pinned","path":"/private/catalog.CTF","cache_root":"/private/cache","generation":"<64 hex characters from ctf_sync>","query":"ext:mp3","limit":100}

Add full_count:true only when exact totals are necessary; otherwise an indexed
first-page query avoids a costly COUNT. An invalid query cannot start an
expensive CTF import. Pinned sessions cannot be passed across independent
atlas request CLI subprocesses and are bounded to 16 generations.

## Correctness and non-goals

Original CTF remains authoritative and never opened for writing. Every
pinned query hashes the source CTF before and after execution; external saves
generate new keys, so old tokens get source_changed. Database size/mtime/inode
changes immediately invalidate the pin; missing/corrupt DB requires a fresh
strict ctf_sync. User annotations and native categories must not be kept in
disposable SQLite derivative generations.

Uncooperative external saves just after a check remain possible without a
cooperative writer lock. A pinned-result generation is a snapshot identity,
not proof of timeless freshness. This is a native service capability, NOT
an integrated compact Windows/macOS GUI workflow or a filesystem watcher.
The original WhereIsIt edited-CTF compatibility gate and remaining CTF
description decoding gap also remain open. No private CTF or derived catalog
bytes are sent to public CI.
