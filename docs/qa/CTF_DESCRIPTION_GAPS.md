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
