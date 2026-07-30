# Validation

No single comparison proves correctness. Use a three-sided fixture record:

| Lane | What it establishes | What it does not establish |
|---|---|---|
| Native XT parse | exact consumption, schema application, raw nodes/fields, indices, pointers, terminator | that the interpreted model has the intended geometry |
| Parasolid 37.01.243 through `parasolid-rs` | official receiver acceptance and public-PK semantic fingerprint | byte-for-byte parser correctness or portability across versions |
| ABCD FeatureScript, metadata, and STEP | construction intent, case classification, and a second representation | exact XT node layout; STEP export can change representation |

FeatureScript is primarily a fixture-selection and intent oracle. It is not an
XT oracle by itself. STEP comparisons are coarse and should tolerate
representation changes while still checking body count, bounding boxes, mass
properties, topology counts, and sampled geometry where stable.

## ABCD acquisition

The local chunk-0 corpus contains 23,093 XT files and 1,000 STEP files. The
existing downloader was also used to acquire the FeatureScript sidecar for
model 896:

```bash
cargo run --manifest-path /home/dev/projects/abcd/Cargo.toml --bin abcd-downloader -- \
  download 896 --formats featurescript \
  --out /home/dev/projects/xt-parser/inputs/abcd
```

That YAML captures the Onshape feature tree and statuses, so it is useful for
intent and fixture classification. It is not a B-rep or XT-layout oracle.

## Current corpus result

The release-mode generated-API gate:

```bash
cargo run --release -p xt-schema --bin xt-schema-inspect -- \
  /home/dev/projects/xt-parser-validation
```

converted 23,093/23,093 files, 893,321,443 bytes, and
14,599,019/14,599,019 nodes with zero errors. Every raw node was converted to
the generated `schemas::sch_30100::Node` enum; therefore the check covers field types and
cardinality as well as raw stream consumption. The earlier raw graph census
also recorded 122,237,529 field values and 69,753,141 pointer values with zero
parse failures.

## Per-fixture manifest

Every comparison record must include:

- model ID and all source-file SHA-256 values;
- parser commit and strict/permissive options;
- Parasolid DLL hash/version and `parasolid-rs` commit;
- header fingerprint and schema name;
- native node/field/type counts, root, terminator, unresolved pointers, and
  unconsumed byte count;
- public-PK receive result, body types, topology counts, bounding boxes, volume,
  area, center of gravity, and stable geometry samples;
- FeatureScript/meta classification and expected feature parameters;
- STEP-derived coarse fingerprint;
- errors with raw spans; and
- comparison tolerances and the reason for each tolerance.

## Gates

1. Hand-authored primitive fixtures: exact raw graph and full consumption.
2. Oracle-generated one-factor fixtures: exact deltas between pairs.
3. Version/schema matrix: embedded-schema handling and old-version conversion.
4. Stratified ABCD subset: zero crashes and categorized failures.
5. Full ABCD corpus: deterministic manifest, zero unexplained trailing bytes,
   and every official-receiver disagreement minimized.
6. Writer: native parse/write/parse graph equality plus Parasolid acceptance and
semantic equality.

## Binary lane

ABCD v00 publishes one `para` archive family, not separate text and binary
families. Its official example ZIP contains three `.x_t` members and no
`.x_b`; the full local model extraction contains 152,096 `.x_t` files and zero
`.x_b`. The downloader already supports `xb` extraction from nested Parasolid
ZIPs when present, and now matches `.x_b`/`.X_B` case-insensitively. ABCD is
therefore not treated as a binary corpus unless an actual binary member is
identified.

Binary fixtures are generated through the documented public PK transmit API.
Six synthetic bodies (block, cube, sphere, cylinder, cone, and torus) were each
transmitted as text, bare binary, neutral binary, and typed binary. The same
three binary encodings were generated after the V37 oracle received 25 and 250
evenly selected ABCD text files. This produced 843 stored binary files:
18 synthetic and 825 ABCD-derived.

All 843 files fully parsed and converted to generated `SCH_37102` Rust types.
For every one of the 281 four-format fixture executions, the original text,
bare, neutral, and typed encodings had equal node counts and complete node-type
censuses. The expanded 250-model batch covered 61,477 nodes per encoding, or
184,431 decoded binary node instances. Its JSON manifest records 250 receives,
750 outputs, and zero oracle errors. Because four input hashes overlap the
pilot, the aggregate has 277 distinct source-text hashes and 828 distinct
binary payload hashes.

The repeatable gate is:

```bash
scripts/check-binary-corpus.sh
```

Separate public-PK transmissions may relabel file indices and reorder otherwise
equivalent nodes. Cross-encoding validation therefore compares node/type
censuses and semantic topology first; raw pointer-index equality is not a valid
standalone gate. `xt-format-compare` records this representation-independent
check.
