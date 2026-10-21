# XT reverse-engineering and parser contract

This is a private, non-clean-room interoperability project. Its purpose is to
produce a native Rust reader and writer for Parasolid XT without linking to
Parasolid at runtime.

## Evidence rules

- Raw bytes are authoritative. Preserve byte offsets and hashes through every
  parser layer.
- Use these claim grades exactly: `documented`, `export-observed`,
  `static-observed`, `dynamic-observed`, `inferred`, and `hypothesis`.
- Never guess node numbers, field order, field widths, schema-version
  correspondence, unset behavior, or conversion semantics. Unknown data is an
  explicit blocker or opaque value.
- The July 2022 XT format reference at
  `/home/dev/old-projects/xt-parser/xt-reference.md` is the primary format
  reference. Its SHA-256 is
  `6d049da8d5ef7735d20db4f9ad004e1ad7f25bb0f78b518e8986a3e434f91157`.
- `/projects/sinbad/parasolid-re/docs/XT_FORMAT.md`, notes under
  `solidworks-notes`, and both quarantined parser implementations are leads
  only. They are not authorities.
- Proprietary manuals, schemas, binaries, licenses, generated Ghidra projects,
  and input models belong under ignored `inputs/` or in their existing private
  repositories. Commit hashes and derived observations, not unreviewed raw
  material.

## Oracle rules

- The canonical runtime target is Parasolid 37.01.243,
  `/projects/sinbad/parasolid-rs/lib/pskernel.dll`, SHA-256
  `c900fa3f430fe67c2adb15a50f9604d9812ed81a02c5006b66689dac28263073`.
- Runtime probes enter only through documented public PK APIs under the user's
  authorized Parasolid environment. Do not call internal functions, patch the
  binary, bypass licensing, or disable integrity checks.
- `parasolid-rs` is an oracle adapter, never a parser dependency. The native
  parser must build and test without a Parasolid installation.
- Record the DLL hash, Parasolid version, `parasolid-rs` commit, probe options,
  fixture hash, and complete output for every oracle run.

## Implementation rules

- Decode the text stream with schema-directed typed reads. Character and logical
  fields have no trailing delimiter, so a generic tokenization layer is invalid.
- Reject unconsumed bytes, malformed delimiters, duplicate node indices,
  unresolved required pointers, impossible embedded-schema edits, and range
  overflows by default.
- Preserve unknown header keys, schema nodes, fields, and raw spans.
- Add a minimal regression fixture for each promoted observation.
- Never hide a parser defect by weakening a test or by accepting arbitrary
  trailing data.
- Keep `STATUS.md` current with evidence hashes, implemented coverage, failed
  experiments, unresolved questions, and the next exact probe.
