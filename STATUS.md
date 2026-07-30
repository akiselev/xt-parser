# Status

Date: 2026-07-29

## Scope and provenance

- New unrelated repository initialized after moving the two legacy repositories
  intact to `/home/dev/old-projects/xt-parser` and
  `/home/dev/old-projects/xt-parser2`.
- Primary format reference SHA-256:
  `6d049da8d5ef7735d20db4f9ad004e1ad7f25bb0f78b518e8986a3e434f91157`.
- Oracle DLL: Parasolid 37.01.243, SHA-256
  `c900fa3f430fe67c2adb15a50f9604d9812ed81a02c5006b66689dac28263073`.
- `parasolid-re` evidence commit:
  `c4b16ca72c8f19260322d6fb008984cee9499035`.
- `parasolid-rs` oracle commit:
  `b580122b07480f067853c872e781c713d73f2742`.
- SolidWorks `SCH_13006` SHA-256:
  `cc98d0c96c959cc0216cee45e4adb785340ed99af7ecb6129f84633a738846af`;
  LF-normalized SHA-256:
  `0dd291ea706fc306f16a78140e05e595e75c85ab63e4077e68941b127fcfdcc3`.
- SolidWorks `SCH_30100` SHA-256:
  `fa7e505b120bf384996060dde4acdb3af2c193efcc64b6fda3813015f8298259`;
  LF-normalized SHA-256:
  `18838cb90466615a57c841896d3842044e97e85f2b5e33b62c41222f4e37ece5`.
- SolidWorks `SCH_37102` SHA-256:
  `7345f7094df516530a7a858d145e80caa80dfbdfe75b8bd51dc7dbc1acc6012d`;
  LF-normalized SHA-256:
  `c2d06b56144524347834699b33d9dbfe7e6a1b8d3cd125b1a4d68593464f8e5f`.

## Findings

- `documented`: text record breaks and trailing record padding are ignored;
  leading spaces remain significant.
- `documented`: ordinary numeric fields have one trailing space; character and
  logical fields do not; `?` represents an unset integral or floating value.
- `documented`: V14+ embedded schema data is a difference from V13
  `SCH_13006`, inserted on first occurrence of each node type; the extended
  schema name is followed by a maximum-node-type short in the internal prefix.
- `documented`: exact local `SCH_13006` and `SCH_30100` files parse to the
  declared inventories of 142 nodes/1,125 fields and 184 nodes/1,402 fields.
- `dynamic-observed`: Onshape/Parasolid 30.1.168 ABCD headers contain
  unescaped spaces in `FRU` values despite the documented `^-` spelling. The
  header parser accepts these with an `unescaped_space` diagnostic.
- `dynamic-observed`: the native parser decoded all 23,093 local ABCD XT files,
  893,321,443 input bytes, 14,599,019 nodes, 122,237,529 field values, and
  69,753,141 pointer values with zero failures. Every file uses
  `SCH_3001168_30100_13006`; the deterministic corpus SHA-256 is
  `b7eb11f60db540257096f0951245bd85d23f88e1a37edc9b25337bb458907acd`.
- `dynamic-observed`: the separate generated `xt-schema` crate converted every
  one of those 14,599,019 raw records to its strongly typed
  `schemas::sch_30100::Node` variant with zero errors.
- `dynamic-observed`: the current `parasolid-rs` public-PK layer transmits and
  receives text XT while preserving tested topology and mass properties.
- `dynamic-observed`: V37.01.243 accepted ABCD fixture SHA-256
  `fd5934c3f05be19f163aae1f5e0cfced44d8d29c5f0e33519ce95c41bc45112a`
  as one solid with 23 faces, 50 edges, 32 vertices, and amount
  `0.1422288445389974`.
- `dynamic-observed`: native parsing of that fixture produced 616 typed nodes
  and exactly matched the public-PK region, shell, face, loop, edge, and vertex
  counts.
- `dynamic-observed`: after normalizing CRLF to LF, the local SolidWorks
  `SCH_13006` and `SCH_30100` files are byte-identical to public copies.
- `inferred`: numbered SCH content is Parasolid-kernel schema metadata rather
  than a SolidWorks/Onshape-specific dialect. Evidence is cross-vendor byte
  identity plus official ABC/Onshape files naming the same schema.
- `dynamic-observed`: the ABCD downloader acquired model 896's FeatureScript
  YAML. It describes feature intent and statuses, not the serialized B-rep.
- `documented`: neutral binary is big-endian IEEE/ASCII; typed binary declares
  byte order, double representation, and character representation; bare binary
  requires out-of-band machine information.
- `dynamic-observed`: the official ABC sample Parasolid ZIP contains three
  `.x_t` members and no `.x_b`. The complete local chunk-0 extraction has
  152,096 `.x_t` and zero `.x_b`; targeted `xb` extraction found no binary
  members. ABCD v00 is not currently a binary fixture source.
- `dynamic-observed`: six public-PK bodies were each transmitted as text, bare,
  neutral, and typed binary. All 18 binary files fully parsed and converted to
  generated `SCH_37102` types; every four-format family had equal node counts
  and node-type censuses.
- `dynamic-observed`: 275 ABCD text-file selections were received by
  V37.01.243 and re-transmitted in bare, neutral, and typed binary. All 825
  generated binary files fully parsed and converted to generated `SCH_37102`
  types, and all four encodings in every family had equal node counts and
  node-type censuses. The pilot and expanded selections overlap by four input
  hashes.
- `dynamic-observed`: the complete stored binary validation corpus contains
  843 files (18 synthetic plus 825 ABCD-derived), 828 distinct binary
  SHA-256 payloads, and 277 distinct source-text SHA-256 values. The expanded
  250-model batch alone exercised 184,431 binary node instances with zero
  failures.
- `dynamic-observed`: typed-binary V37 uses descriptor `1,0,0`
  (little-endian, IEEE, ASCII); bare V37 on the Windows x64 oracle uses the same
  machine representation. Neutral uses the documented big-endian IEEE/ASCII.
- `dynamic-observed`: separate transmissions of the same body can relabel node
  file indices, so raw pointer-index equality across format variants is not a
  valid semantic comparison.
- `inferred`: repeated node types whose `next` or `previous` field targets the
  same abstract pointer class form an inspectable schema family. Applying this
  rule without a node taxonomy recovers surface, curve, attribute/feature, and
  schema-node-map families in `SCH_37102`; labels come from the most frequent
  non-link field referencing each class.

## Implemented

- Evidence and validation contracts.
- Common-header parser.
- Text physical-to-logical offset map and typed primitive cursor.
- Text internal-prefix parser.
- Strict `.s_t` schema importer and embedded-schema edit validation.
- Complete schema-directed text node graph parser and recursive corpus census.
- Separate, deterministic, unit-tested `xt-schema-gen` generator crate.
- Separate generated `xt-schema` crate containing embedded 13006/30100 parser
  and 37102 parser metadata, typed scalar/cardinality fields, pointer targets,
  node structs, schema-wide enums, and convenience parsing APIs.
- Generated modules isolate identity metadata, parser specifications, pointer
  targets, inferred hierarchy, family records, and standalone records.
- Schema-driven family inference and labels are fully automatic; the generator
  contains no manually specified node IDs, node names, or semantic groups.
- Generated-code drift check and a recursive typed corpus checker.
- Bare, neutral, and typed-binary envelope/cursor support sharing the full
  schema-directed node graph engine with text.
- Binary inspection and cross-format node-census comparison tools.
- Parameterized `xt-oracle` CLI in `parasolid-rs`, cross-compiled and exercised
  under Wine against the canonical DLL.
- Public-PK `xtdump` fixture generator now emits text plus all three binary
  encodings with valid format headers.
- Public-PK `xt-reencode` batch generator deterministically samples a text
  corpus, receives each file, emits all three binary encodings, and records
  input/output hashes and failures in JSON.
- Reproducible manifest-driven cross-format validation accepts both ABCD
  `SCH_30100` text and current `SCH_37102` text, then requires equal node counts
  and complete node-type censuses across text, bare, neutral, and typed binary.
- ABCD downloader `.x_t`/`.x_b` ZIP-member matching is case-insensitive.

## Remaining work and next probes

1. Generate one-factor public-PK fixtures for every field representation,
   especially currently unobserved schema codes `q` and `t`.
2. Build typed graph-resolution and topology/geometry views over
   `XtPointer<T>` indices without discarding raw spans.
3. Run a stratified ABCD comparison ledger across native parsing,
   `xt-oracle`, FeatureScript metadata, and STEP, with explicit tolerances.
4. Implement canonical text writing, then require native parse/write/parse
   equality and official Parasolid receiver acceptance.
5. Add VAX D-float/EBCDIC only after obtaining matching typed-binary fixtures;
   add other bare-machine layouts only with explicit provenance.
6. Implement binary writing and older/newer generated schema modules from exact
   inputs; standalone V13 stream handling is not yet implemented.
