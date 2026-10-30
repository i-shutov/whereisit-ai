# Description metadata coverage in Catalog 3.00

**Source policy:** private CTF bytes, names, descriptions and source paths are
never placed in the public repository, its Actions runners or their artifacts.

The CTF parser deliberately preserves unknown extension bytes even when
they cannot be converted into searchable Unicode text. A byte-identical
unchanged CTF roundtrip therefore does not imply description-search parity.

## Diagnostic categories

`ctf_format::diagnostics` retains the old
`unsupported_descriptions` field for backwards compatibility: the number
of entries with an empty decoded description and at least one type-1
description extension whose declared length is nonzero.

It now also returns two aggregate maps:
`description_gap_reasons` and `description_gap_flag_histogram`.
Only *numeric flags* and reason labels are exposed. Values count type-1
extensions inside candidate entries, so their total can exceed the
number of candidate entries if a record contains multiple extensions.

- `unsupported_high_flags`: top eight bits outside the known 0x04/0x08 masks
- `external_reference_not_diagnosable_from_entry`: description pointer
  refers to a source-level blob; entry-only opaque bytes cannot prove whether
  the pointer was valid or invalid
- `invalid_inline_utf8_or_cesu8`: inline bytes fail the existing strict
  Unicode decoder
- `explicit_empty_inline`: inline extension decodes to the empty string
- `decoded_inline_but_final_description_empty`: an inline string is valid
  but the final recorded description is empty (e.g., multiple fields or a
  parser handling issue); this should be investigated rather than discarded

**Do not** label every candidate field as corruption. A nonzero declared
length could legitimately represent the NUL terminator for an empty string.
No decoding semantics were changed by this diagnostics-only patch.

## Running on a private machine

Download the `ctf-description-probe-<commit>` artifact from the dedicated
privacy-safe GitHub Actions workflow, verify `SHA256SUMS.txt` and provenance,
then run:

```sh
./description_gap_probe '/path/to/private/USB-drives.CTF' > private-description-aggregates.json
```

Keep the resulting report local, especially if it includes a source digest
added by your own wrapper. The executable prints only aggregate counts and
labels; it does not print file names, paths, serials, or descriptions.
It must be run with sufficient memory to parse the CTF snapshot, and it
does not modify the original source.

## Acceptance gate

Before claiming complete compatibility, the flagged high-byte classes and
failed inline encodings must be examined using independently generated
synthetic fixtures or authoritative original-app behavior. No speculative
fallback Unicode decoder should be introduced into the CTF writer.
The existing 44-disk file's byte-exact CTF roundtrip must remain true and
the count of genuinely missing searchable text must be remeasured.

Related: [Issue #23](https://github.com/i-shutov/whereisit-ai/issues/23).

## 2026-10-30: independent evidence changes interpretation

The previous legacy `unsupported_descriptions` field is **not evidence
that the same number of user-authored descriptions was lost**. A second,
independent read-only scan of the same 3,046,027-entry real CTF found all
1,782 candidate type-1 fields with high flag `0x1c`. Of these, 1,688
pointers landed inside the assumed external metadata blob but none of the
referenced payloads was ordinary valid UTF-8 text. The remaining 94 pointers
did not fit the currently assumed text-blob addressing model. Binary
length/header-like patterns dominate, including payload sizes 277 and
293 bytes.

The extra bit `0x10` is **not yet defined by an original-app-derived
specification**. These fields could represent binary/plugin metadata or
an unknown encoded text format. It is incorrect to discard them *or*
blindly interpret their bytes as UTF-8. The parser currently keeps them
opaque and preserves byte-exact unchanged CTF roundtrips.

New additive diagnostic counters keep the legacy combined field unchanged:

- `opaque_type1_flag_0x10_entries`: per-entry count for nonempty type-1
  extensions with the unresolved 0x10 high flag and no decoded description
- `possible_text_description_gap_entries`: per-entry count for nonempty
  type-1 extensions without 0x10 that still have an empty decoded description

An entry with multiple type-1 extensions may count in **both** new buckets;
the legacy combined field counts the entry once. The reason label
`opaque_type1_flag_0x10_semantics_unknown` is deliberately cautious:
it does NOT assert compression, encoding, proprietary plug-in structure or
loss of searchable user text.

Only numeric aggregates and synthetic fixture bytes appear in GitHub.
Original names, descriptions, raw CTF, source SHA and disk labels remain local.
True completeness still requires observing these fields in original WhereIsIt
or obtaining an authoritative specification for bit 0x10; this classification
patch does not close the compatibility gate.
