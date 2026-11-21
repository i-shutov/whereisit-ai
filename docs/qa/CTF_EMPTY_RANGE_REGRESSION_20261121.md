# Empty catalog and absent-range refusal regression

Independent acceptance of main `8cbb3b4f5e9f07537cbd0d42deeeee01ec4e35ef` found two malformed-input cases where parsing succeeded and rewriting silently normalized unaccounted bytes or contradictory fields:

1. Append one arbitrary byte to a generated catalog with zero blocks. There is no offset-owned block to account for the suffix.
2. In a generated disk with an absent root-file range, retain the absent offset/count/blob tuple but change the length sentinel or name-byte count.

These are synthetic parser refusal defects. No loss from a valid vendor catalog, source modification, crash or security exploit was demonstrated. They are tracked as a P1 data-preservation acceptance blocker for malformed inputs, separately from the previously fixed vendor CTF interoperability defects.

The parser now requires the end of an empty catalog header to equal the file length. An absent range must have the expected length sentinel and zero name bytes, in addition to the existing zero file-count/blob checks. Correct empty catalogs and present metadata-only ranges still roundtrip. The prior present-range metadata length/name guard was already correct and is unchanged.

Red/green validation: both new tests failed on the main parser; all 15 format tests passed after the checks. The combined format/service library and integration suites passed 98 tests. Tests construct their own bytes and use no private data. This local follow-up does not change the final-main benchmark binary or approve stable 1.0.
