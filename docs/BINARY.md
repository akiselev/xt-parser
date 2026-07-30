# Binary XT

The native binary reader uses the same schema metadata, embedded-schema edit
engine, raw graph, and generated node types as the text reader.

## Encodings

The body immediately after the common text header begins with one of:

| Kind | Signature | Machine representation |
|---|---|---|
| Bare | `B` | Not self-describing; caller must supply it |
| Neutral | `PS 00 00` | Big-endian, IEEE doubles, ASCII |
| Typed | `PS 00 01` plus three descriptor bytes | Declared byte order, double representation, and character representation |

Neutral and typed IEEE/ASCII files are automatically decoded. Bare files are
rejected unless the caller supplies a `MachineEncoding`; this avoids guessing
from the current host. VAX D-float and EBCDIC descriptors are preserved in the
prefix result but rejected for node decoding.

Binary node types and shorts are 16-bit, integers are 32-bit, and IEEE doubles
are 64-bit in the declared byte order. Pointer indices and embedded-schema
positive integers use the documented compact encoding: values below 32,767 use
one short, while larger values use a negative remainder and positive quotient.
The binary terminator is node-type short `1` followed by compact index `0`
(encoded as short `1`).

## Generated API

`xt-schema` embeds `SCH_37102` and exposes:

- `parse_binary_37102` for neutral and typed binary;
- `parse_binary_37102_with_machine` for explicitly described bare binary;
- `TypedBinaryDocument<schemas::sch_37102::Node>`; and
- `schemas::sch_37102::{metadata, spec, pointers, hierarchy, nodes}`.

Inspect one file with:

```bash
cargo run -p xt-schema --bin xt-binary-inspect -- model.x_b

cargo run -p xt-schema --bin xt-binary-inspect -- \
  --bare-little-ieee-ascii model.x_b
```

Compare public-PK text/binary fixture families with:

```bash
cargo run -p xt-schema --bin xt-format-compare -- \
  model_text.xmt_txt \
  model_bare.xmt_bin \
  model_neutral.xmt_bin \
  model_typed.xmt_bin
```

## Current evidence and limits

The checked corpus contains:

- six public-PK-created shape families in text, bare little-endian IEEE/ASCII,
  neutral, and typed little-endian IEEE/ASCII: 18 binary files;
- a 25-model ABCD pilot re-encoded through the public PK API: 75 binary files;
  and
- an evenly selected 250-model ABCD batch re-encoded the same way: 750 binary
  files.

All 843 stored binary files fully decoded and instantiated generated
`SCH_37102` types. Every one of the 281 exercised four-format families matched
its source text in node count and complete node-type census. Four input hashes
occur in both ABCD batches, so the aggregate contains 828 distinct binary
SHA-256 payloads derived from 277 distinct source-text SHA-256 values.

The expanded batch manifest is
`inputs/oracle-binary-abcd-250/manifest.json`; it records 250 successful
receives, 750 binary outputs, their hashes and sizes, and zero failures. Rerun
the complete checked corpus with:

```bash
scripts/check-binary-corpus.sh
```

This does not yet establish VAX D-float, EBCDIC, other bare-machine layouts,
binary writing, or every possible field representation. Binary and text
transmissions of the same live body can assign different file indices, so exact
pointer-number equality across independently transmitted files is not expected;
semantic graph comparison must account for index relabeling.
