# Status

Date: 2026-10-21

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

## R-52/R-49 fixtures lane (2026-10-21, same branch)

- `dynamic-observed`: 39 of 39 R-52 unusual bodies (RE `0dacf02c`) and 5 of 5 R-49
  minimal bodies (RE `9a971ec3`) parse and match the kernel's counts
  (`cargo test -p xt-schema --test pk_fixtures`, 4 tests), and 44 of 44 pass the
  native-versus-live-Parasolid comparison (`oracle/run.sh`, parasolid-rs `f889c61`).
  Fixtures are vendored in `crates/xt-schema/tests/fixtures`.
- CLIs now dispatch on the file's schema: `xt-schema-inspect`, `xt-native-dump`,
  `xt-show`, `xt-embedded-check` (`xt_schema::parse_text`).
- Non-embedded ("complete") text form and headerless bare streams parse
  (`XtDocument::parse_text` chooses by the schema name; `TextEnvelope::header` is an
  `Option`). Unit tests: `complete_stream_*`.
- View corrections from the fixtures: `Fin::vertex` renamed `end_vertex` (it is the
  vertex at the end in loop order; R-39 section 2); an acorn shell has one vertex
  (`Shell::acorn_vertex`, a vertex chain would run into the next shell);
  `Face::surface` and `Vertex::position` are `Option` (missing geometry is a valid
  receive state); `BodyKind::General` added. Wireframe edges (`Shell::wire_edges`)
  and loop-less partner fins were already handled before RE read 123d09a.
- Comparison: stored kinds wire/general with no edges/shells equal Parasolid's
  derived acorn, minimum and empty; `bounds` unavailable from Parasolid for an empty
  body or a curveless wire is recorded, not a failure.

### Design for a cadabra-to-XT writer (not built in this lane)

Target: the complete form `SCH_3701243_37102` (transmit_version 371), delivered with
`parasolid::fileio::receive_from_str`. The reader side is now ready for round trips.
Proposed `xt-schema` API (per generated schema, so 37102 only at first):

1. `write::TextWriter::new(schema_version = 3701243, res_size, res_linear)` and
   `fn write(&self, nodes: &[sch_37102::Node]) -> Result<String, WriteError>`; the
   caller builds typed nodes with explicit indices (BODY first) and the writer only
   serialises: `Node -> NodeRecord` (the inverse of `TryFrom<&NodeRecord>`, generated
   next to it by `xt-schema-gen`), then a `TextSink` that is the inverse of
   `TextCursor` (numbers end in one space, `?` for unset, c/l fields without a space,
   strings length-prefixed, shortest-round-trip reals).
2. `WriteError`: pointer to a missing index, duplicate index, non-unit direction
   vector, vertex tolerance below an incident edge tolerance, tolerant edge with a
   curve or without fin curves, `res_size`/`res_linear` unequal to the session.
   Each is a typed error found before emission (R-49 section 4 to 6 rules).
3. Line breaking at no fixed width but never before a space (no physical line may end
   in a space); simplest is one logical line, optionally wrapped at 200+ columns on a
   non-space character.
4. A `view`-shaped builder (`Builder` that allocates indices and owner/next/previous
   chains for BODY, REGION x2, SHELL, FACE, LOOP, HALFEDGE, EDGE, VERTEX, geometry)
   belongs in the cadabra-side crate; xt-parser stays a vocabulary and serialiser.
5. Round-trip test: parse each r49 fixture, write, parse again, compare node by node
   (indices may be renumbered), and receive the written text in Parasolid.

## Oracle cross-check lane (branch `lane/oracle-xcheck`, 2026-10-21)

Machine paths that replace the unavailable `/home/dev/...` ones: parasolid-rs
`/projects/sinbad/parasolid-rs` (master `f889c611d05986836b08a32549631945770cdedd`
for the pinned runs below), DLL `/projects/sinbad/parasolid-rs/lib/pskernel.dll`
(SHA-256 `c900fa3f...3073`, matches the canonical hash), downloader
`/projects/sinbad/abcd-downloader`, corpus `/projects/sinbad/xt-corpus/abc/` (outside every repo).

- `dynamic-observed`: ABC chunk 0 (`abc_0000_para_v00.7z`, 10,000 models) holds
  152,096 `.x_t` and one `.xmm_txt` member (not extracted by the downloader).
  Every `.x_t` member name is lower case, so the downloader's case-sensitive
  `unzip_para` match (`src/lib.rs:545`) loses nothing on this chunk. It is
  unchanged. STEP files for the same ids (10,000, one per model) are alongside.
- `dynamic-observed`: the whole chunk was described by the native parser and by
  Parasolid and compared file by file (tables below): 152,085 pass, 0 fail,
  0 native errors, 11 oracle-silent (Parasolid rejects the file).
- Resolved B-rep view added in `xt-schema::view` (docs/GENERATED-API.md).
- Cross-check tools in `oracle/` (docs/ORACLE.md).
- Embedded-schema selection (`parse_text_embedded`, `parse_binary_embedded`):
  layouts come from the file's own edits against the 13006 base; a new node type
  needs the newest schema (37102) to name it and say whether it is variable
  length, else the parse fails. Records equal the exact-schema parse on 2,002
  SCH_30100 files and on 41 SCH_37102 files (`oracle/target/release/xt-embedded-check`),
  and four synthetic neutral-binary unit tests (edited layout, exact rejection,
  unknown new type, wrong base).

### Native bugs found and fixed by the comparison

1. Text Unicode code units are written as signed shorts (`-239` is U+FF11,
   `-31541` is U+84CB). The reader required 0..65535 and failed 8 files with
   CJK or full-width part names. Fixed in `TextCursor::read_u16`, test
   `unicode_code_units_are_signed_shorts_in_text`.
2. View-level: trimmed-curve wrappers, isolated-vertex loops, loop-less partner
   fins, wire edges and isolated vertices owned by shells, face sense
   combined with surface sense (all listed in docs/ORACLE.md).

### Results (pinned run: parasolid-rs f889c61, DLL c900fa3f, xt-parser b05037b)

Per-run provenance is in `/projects/sinbad/xt-corpus/runs/final/*/provenance.txt`;
per-file TSVs in `final/run.NN/result.tsv`, `final/sample2000/result.tsv`.

| Set | Files | Pass | Fail | Oracle silent | Native error |
|---|---|---|---|---|---|
| Stratified sample (every 76th file of the sorted chunk list) | 2,002 | 2,002 | 0 | 0 | 0 |
| Whole chunk 0 (16 shards) | 152,096 | 152,085 | 0 | 11 | 0 |

Compared in the chunk: 152,096 bodies (63,474 solid, 85,605 sheet, 3,017 wire;
no multi-body file), 217,359 regions, 219,183 shells, 2,944,959 faces,
3,592,526 loops, 7,049,226 edges, 4,219,014 vertices, 13,593,437 fins. Surface
classes: plane 1,745,615, cylinder 612,111, cone 99,464, sphere 27,874, torus
92,406, B-spline 244,559, blend 51,841, swept 43,583, spun 24,007, offset 3,498,
mesh 1. Edge carriers: line 3,916,775, circle 1,342,983, ellipse 111,281,
B-spline 320,939, intersection 294,247, SP-curve 8,916, none (tolerant) 1,054,085.
Tolerant vertices 1,034,931. Every entity paired by node identifier.
Native body type codes `1/2/3` were found to be solid/wire/sheet by the
comparison (code table in `BodyKind`); other codes are an error.

Oracle-silent triage (never counted as passes):

- 10 files `CarbSpacer parts - Curve 1..10.x_t` (model 859): `PK_PART_receive`
  returns error 922 (`corrupt_file`). Each is a one-edge wire body on a closed
  rational degree-2 B-curve (85 control points, C0 knot multiplicities); the
  native parse is complete and consistent. Genuine file oddity or kernel
  refusal; not a native defect. Needs RE input to say which kernel check fires.
- 1 file `Part Studio 3 - Mesh 1.x_t` (model 5817): error 5237
  (`facet_geometry`) on a body whose face carries a mesh surface. Probable
  `parasolid-rs` receive-options limit for mesh geometry.

### Commands (all cargo through `/projects/sinbad/cadabra3/scripts/cargo-locked`)

```
# downloader build and acquisition
cd /projects/sinbad/abcd-downloader
CARGO_TARGET_DIR=/projects/sinbad/xt-corpus/build CARGO_BUILD_BUILD_DIR=/projects/sinbad/xt-corpus/build/b \
  cargo-locked build --release
abcd-downloader download chunk:0 --formats xt   --out /projects/sinbad/xt-corpus/abc --cache-dir /projects/sinbad/xt-corpus/cache
abcd-downloader download chunk:0 --formats step --out /projects/sinbad/xt-corpus/abc --cache-dir /projects/sinbad/xt-corpus/cache
# lists
cd /projects/sinbad/xt-corpus && find abc -name '*.x_t' | sort > all.list
awk 'NR%76==1' all.list | sed 's|^|/projects/sinbad/xt-corpus/|' > runs/sample2000.list
sed 's|^|/projects/sinbad/xt-corpus/|' all.list > runs/all.abs.list && split -n l/16 -d runs/all.abs.list runs/final/list.
# tools
cd /projects/sinbad/xt-parser/oracle
cargo-locked build --release -p xt-native-dump -p xt-xcheck-compare
cargo-locked build --release --target x86_64-pc-windows-gnu -p xt-pk-dump
# run (native, Parasolid under Wine, compare, provenance)
oracle/run.sh LIST RUN_DIR
```

### Blocked: schema selection on a third version

The Parasolid side cannot write other versions here. `PK_PART_transmit` with
`transmit_version` 260..370 returns 970 (`schema_access_error`); the frustrum
trace shows it opening `SCH_30000.sch_txt` (guise schema) in the base directory
for version 300. Version 371 (the current 37.1) works, other numbers return 923.
No `SCH_<version>.sch_txt` file exists on this machine (the RE session's
scratch holds `sch_8000..9100.sch_txt`, versions 8 to 9 only). Needed from the RE
session: schema files for versions between 14 and 37 (at least 30000, 33000,
36000) staged where the oracle's frustrum base directory can read them, or a
`parasolid-rs` way to produce them. Meanwhile embedded selection is verified on
the two real versions and on synthetic edits only.


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
- ABCD downloader `.x_t`/`.x_b` ZIP-member matching is case-insensitive in the
  earlier copy; the checkout at `/projects/sinbad/abcd-downloader` still compares
  case-sensitively (no effect on chunk 0, see the cross-check section).

## Remaining work and next probes

1. Generate one-factor public-PK fixtures for every field representation,
   especially currently unobserved schema codes `q` and `t`.
2. (done 2026-10-21) Typed graph-resolution and topology/geometry view:
   `xt_schema::view`. Raw spans stay on the underlying nodes.
3. (partly done) Native/Parasolid comparison ledger over all of chunk 0 is in
   `oracle/`; still open: FeatureScript metadata, STEP, mass properties.
4. Implement canonical text writing, then require native parse/write/parse
   equality and official Parasolid receiver acceptance.
5. Add VAX D-float/EBCDIC only after obtaining matching typed-binary fixtures;
   add other bare-machine layouts only with explicit provenance.
6. Implement binary writing and older/newer generated schema modules from exact
   inputs; standalone V13 stream handling is not yet implemented.
