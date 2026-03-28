# xt-winnow — Parasolid PS30+ XT Text Format Parser

Winnow-based parser for the Parasolid `.x_t` compact transmit format (PS 30+). Produces B-Rep topology (bodies, shells, faces, loops, fins, edges, vertices) and geometry (planes, cylinders, cones, spheres, tori, NURBS). Cross-validated against STEP ground truth from the ABC dataset.

## Quick Reference

```sh
cargo test                                      # unit tests
cargo run --example parse_xt -- file.x_t        # parse + print topology
cargo run --example validate -- dir/            # batch stats
cargo run --example dump_types -- file.x_t      # entity type breakdown
```

## Conventions

- **Rust Edition 2024 / 1.85+**.
- **Fail-fast on unknowns.** Unknown entity types or field codes are parse errors, never silently skipped.
- **No placeholders or stubs.** Implement fully or don't implement at all.
- **Error context mandatory.** Every fallible op chains `.context()` (anyhow).

---

## XT File Format Specification

### File Layout

```
Line 1:  **ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz**...   (charset validation)
Line 2:  **PARASOLID !"#$%&'()*+,-./:;<=>?@[\]^_`{|}~0123456789**...   (charset validation)
Line 3:  **PART1; MC=...; FRU=Parasolid 30.1.168; APPL=SolidWorks; FORMAT=text; ...
Line 4:  **PART2; SCH=SCH_3001168_30100; USFLD_SIZE=0;
Line 5:  **PART3;
Line 6:  **END_OF_HEADER***...     (padded to fixed width)
         <body — all tokens whitespace-delimited, newlines are just whitespace>
```

### PART1 Key-Value Pairs

| Key | Example | Meaning |
|-----|---------|---------|
| `FRU` | `Parasolid 30.1.168` | Frustrum (PS version string) |
| `APPL` | `SolidWorks` / `Onshape` | Writing application |
| `FORMAT` | `text` | Always "text" for X_T |
| `GUISE` | `transmit` | Always "transmit" |
| `DATE` | `2018-04-27T08:24:19 (UTC)` | ISO 8601 timestamp |

### Schema Key (PART2)

`SCH_{modeller_version}_{schema_version}` where modeller_version = major*100000 + minor*1000 + patch.

### T-line (first token after header)

```
T51 : TRANSMIT FILE created by modeller version 300116823 SCH_3001168_30100_13006
```
- `T` = text transmit guise
- `51` = internal format version (0x33)
- `300116823` = packed version: major*10000000 + minor*100000 + patch*1000 + build
- Schema key identifies the base schema

### Schema Preamble (after T-line comment)

Compact inline schema defining entity types present in this file. Format:
```
<n_types> <n_secondary> <annotation_chars><name_len> <type_name> <type_id> <flags> ...
```
See `~/cadatomic/solidworks/notes/xt/schema_preamble_text_parser.md` for definitive format.

### Z1 Record (entity list header)

```
Z1 <n_entities> <body_count> <partition_count> 0 0 0 0 0 0 <res_size> <res_linear> 0 <root_indices...>
```
- `n_entities` = total entities in file
- `res_size` = size resolution (typically 1e3)
- `res_linear` = linear tolerance (typically 1e-8)
- Root indices = BODY, SHELL, REGION references

### Entity Stream

After Z1, entities are written as a flat token stream (newlines = whitespace):

```
<type_id>                         uint16 decimal
<inline_schema>                   ONLY on first occurrence of this type_id
  Path A (has base): <n_new_fields> <annotation_chars> Z
  Path B (no base):  <n_fields> <type_name> <alias> <field_descriptors...>
<version_int>                     ONLY if entity is variable-length
<entity_index>                    int32 (1-based entity handle)
<field_values...>                 per schema field descriptors
```

Terminator: `type_id == 1` followed by partition_idx.

### Field Types in Stream

| Type | Encoding | Notes |
|------|----------|-------|
| Integer (d/u/n) | Decimal: `12`, `-1`, `0` | |
| Float (f) | Decimal: `3.14159265358979324`, `1e-10`, `.004572` | 17 sig figs for round-trip |
| Pointer (p) | Integer index: `0` (null), `42` (entity 42) | NOT `#N` in compact format |
| Logical (l) | `T`/`F` characters, may be packed: `FFF1` | |
| Character (c) | Single char: `F`/`R` (face sense), `+`/`-` (edge sense), `S`/`V` (region) | |
| Vector (v) | 3 consecutive floats (no parens) | |
| Optional pointer | `?N` = optional pointer to entity N | `?` prefix |
| Optional float | `?` alone = NaN sentinel | field type determines interpretation |
| Array | Count determined by version_int or fixed schema | variable-length |

### Entity Types (Topology)

| ID | Name | Key Fields |
|----|------|------------|
| 3 | PARTITION | top-level container |
| 10 | ASSEMBLY | collection of instances |
| 11 | INSTANCE | body/assembly placement with transform |
| 12 | BODY | shell, region, surface/curve/point lists, res_size, res_linear |
| 13 | SHELL | body ref, first face |
| 14 | FACE | shell, loop, surface ref, sense (F/R), tolerance |
| 15 | LOOP | first fin (halfedge) |
| 16 | EDGE | start/end vertex, curve ref, tolerance |
| 17 | FIN (halfedge) | edge, next/prev in loop, sense (+/-) |
| 18 | VERTEX | point ref, tolerance |
| 19 | REGION | shell ref, type (S=solid, V=void) |
| 29 | POINT | 3D coordinates (v) |

### Entity Types (Analytic Geometry)

| ID | Name | Parameters | Math |
|----|------|------------|------|
| 30 | LINE | pvec(v), direction(v) | C(t) = pvec + t*direction |
| 31 | CIRCLE | centre(v), normal(v), x_axis(v), radius(f) | C(t) = centre + r*(cos(t)*x + sin(t)*(n x x)) |
| 32 | ELLIPSE | centre(v), normal(v), major_axis(v), semi_minor(f), semi_major(f) | |
| 40 | PARABOLA | | |
| 41 | HYPERBOLA | | |
| 50 | PLANE | point(v), normal(v) | (P - point) . normal = 0 |
| 51 | CYLINDER | pvec(v), axis(v), ref_dir(v), radius(f) | |
| 52 | CONE | apex(v), axis(v), ref_dir(v), half_angle(f) | |
| 53 | SPHERE | centre(v), radius(f) | |
| 54 | TORUS | centre(v), axis(v), major_r(f), minor_r(f) | |

### Entity Types (NURBS/B-Spline)

| ID | Name | Notes |
|----|------|-------|
| 43 | BSPLINE_CURVE | legacy B-spline |
| 45 | BSPLINE_VERTICES | control point array (n_vertices * vertex_dim doubles) |
| 124 | B_SURFACE | → NURBS_SURF(126) |
| 126 | NURBS_SURF | u_degree, v_degree, control grid, knots |
| 127 | KNOT_MULT | knot multiplicity array |
| 128 | KNOT_SET | distinct knot values |
| 134 | B_CURVE | → NURBS_CURVE(136) |
| 136 | NURBS_CURVE | degree, n_vertices, vertex_dim, knot data |

Control point layout for surfaces: U varies fastest. Rational NURBS (dim=4): homogeneous [x*w, y*w, z*w, w].

### Entity Types (Special Geometry)

| ID | Name | Notes |
|----|------|-------|
| 46 | OFFSET_CURVE | offset of curve by distance |
| 55 | OFFSET_SURF | offset of surface |
| 67 | SWEPT_SURF | surface swept along curve |
| 68 | SPUN_SURF | surface of revolution |
| 132 | PCURVE | curve in surface parameter space |
| 137 | SP_CURVE | curve on surface with chart |

### Entity Types (Attributes)

| ID | Name | Notes |
|----|------|-------|
| 70 | ATTRIBUTE | entity → attribute chain, 9 fields in PS30 (8 in sch_13006) |
| 71 | ATTRIB_DEF | attribute definition, multi-element fields (actions×8, legal_owners×14) |
| 82-89 | ATTRIB_*_VALUE | variable-length attribute value entities |

### Common Field Pattern (all curves/surfaces)

```
node_id              d    entity tag
attributes_features  p    → ATTRIBUTE chain
owner                p    → body/shell geometry list
next                 p    → next in owner's list
previous             p    → prev in owner's list
geometric_owner      p    → shared geometry ref (PS 7002+)
sense                c    '+' or '-'
<type-specific fields...>
```

### Non-Transmitted Fields

Fields with `transmit_flag=0` in sch_13006 are NOT in the stream — recomputed on load:
- `face_box`, `body_box` (bounding boxes)
- `body_box_tightness`
- `type` on LOOP/FACE (inferred from topology)
- `u_int`, `v_int` on FACE (UV parameter domain)
- CURVE_DATA, SURFACE_DATA caches

### Cross-Reference Resolution

Two-pass deserialize:
1. Read entities sequentially, store pointer fields as raw int32 indices
2. Patch all pointer fields: int32 index → entity reference

Forward references (entity N → entity M where M > N) are legal and common — topology is cyclic (face→loop→fin→edge→face).

---

## Architecture

```
header.rs     — parse header lines, extract version/app/schema key
schema.rs     — parse_tline(), parse_schema_preamble(), ps13_schema() base schemas
entity.rs     — parse_entities(): type_id → inline schema → entity_idx → fields
token.rs      — low-level winnow parsers for floats, ints, pointers, logicals
types.rs      — FieldType, EntitySchema, RawEntity, FieldValue enums
build.rs      — build_bodies(): raw entities → typed Body/Face/Edge/Vertex IR
error.rs      — error types
lib.rs        — public API: parse_xt() → Vec<Body>
```

---

## Schema System

Base schemas from `~/cadatomic/solidworks/SOLIDWORKS/data/pschema/sch_13006.s_t`. This is the base for the ABC dataset's `SCH_3001168_30100_13006`.

### Reading sch_13006.s_t

Each entity block:
```
<type_id> <type_name> <n_fields> <parent_type_id>
  <field_name>; <field_type>; <transmit_flag> <extra1> <extra2>
  ...
```

Key columns:
- `transmit_flag=0` → NOT in stream, skip
- `transmit_flag=1` → in stream, parse
- `extra2=0` → scalar field
- `extra2=N>1` → N elements per field (multi-element: P2, F9, etc.)
- `extra2=1` → variable-length array (entity is variable-length)

### Multi-element fields

| Entity | Field | Schema | FieldType |
|--------|-------|--------|-----------|
| INTERSECTION | surface | `p; 1 1006 2` | `P2` (2 pointers) |
| BLENDED_EDGE | surface | `p; 1 1006 2` | `P2` |
| TRANSFORM | rotation_matrix | `f; 1 0 9` | `F64x9` |
| ATTRIB_DEF | actions | `u; 1 0 8` | 8 × uint8 reads |
| ATTRIB_DEF | legal_owners | `l; 1 0 14` | 14 × T/F reads |

### Variable-length entities

Last field has `extra2=1` → entity is variable-length. The VERSION int (read before entity_idx) IS the array element count.

For entities with a fixed V (h-type) field already in base (CHART, LIMIT): `var_count = version - 1`.

Types 82-89 (attribute value entities) are the main variable-length types.

### PS30-specific types (Path B)

Types NOT in sch_13006 get full inline schema (Path B). Currently known:
- Type 204 (INTERSECTION_DATA) — removed from ps13_schema, uses Path B

---

## Ghidra Reverse Engineering

Ghidra project `solidworks` has `pskernel.dll` imported and analyzed (42K functions).

### Key Addresses

| Address | Name | Purpose |
|---------|------|---------|
| `0x180a24ab0` | pk_receive_entity_typed | Main entity read: schema → version(conditional) → entity_idx → fields |
| `0x180a27d80` | pk_read_inline_schema | Parse Path A/B inline schema annotations |
| `0x180a1dbe0` | pk_read_field_data | Type dispatch for reading field values |
| `0x180a1ff90` | field_element_count | array_count=0→scalar, N>1→N elements, 1→variable |
| `0x182054680` | text_read_tag_ptr | Read integer (entity pointer / tag) |
| `0x182054480` | text_read_float | Read float, `?` = NaN sentinel |
| `0x1820540c0` | text_read_raw_byte | Read single raw byte |
| `0x182054fa0` | text_read_logical | Read T/F logical |
| `0x1845ca580` | global_schema_table | Runtime schema (not dumpable from static binary) |

### Ghidra Commands

```sh
# Open pskernel.dll
ghidra program open --program pskernel.dll --project solidworks

# Decompile key functions
ghidra decompile 0x180a24ab0   # entity read main loop
ghidra decompile 0x180a27d80   # inline schema parser
ghidra decompile 0x180a1dbe0   # field type dispatch
ghidra decompile 0x180a1ff90   # element count logic
ghidra decompile 0x182054680   # text tag/ptr reader
ghidra decompile 0x182054480   # text float reader

# Rename a function you've identified
ghidra rename 0x<addr> my_function_name --project solidworks --program pskernel.dll

# Search for related functions
ghidra xrefs 0x<addr> --project solidworks --program pskernel.dll
ghidra search "receive" --project solidworks --program pskernel.dll
```

### Key Ghidra Findings

- **has_version**: set when schema descriptor last field has `array_count == 1`
- **T-flag and extra flags**: ONLY read when `has_handle_map != 0` — skipped for text format transmit
- **Text format**: `?` prefix on pointer fields = optional. `?` alone in float fields = NaN.
- **Entity terminator**: type_id == 1, followed by partition index

---

## Validation Workflow: STEP Cross-Check

This is the primary development loop. Parse XT, compare topology counts against STEP ground truth, fix mismatches.

### Data Sources

- **XT files**: `~/cadatomic/xt-parser/test-data/abc/xt_files/` (extracted .x_t from ABC dataset)
- **XT archives**: `~/cadatomic/xt-parser/test-data/abc/extracted/<model_id>/*.zip`
- **STEP files**: `~/cadatomic/xt-parser/test-data/abc/step/` (download from NYU archive)
- **Schema files**: `~/cadatomic/solidworks/SOLIDWORKS/data/pschema/sch_13006.s_t`

### Setup (one-time)

```sh
# Download ABC STEP chunk 0000
cd ~/cadatomic/xt-parser/test-data/abc/
wget --no-check-certificate "https://archive.nyu.edu/rest/bitstreams/88598/retrieve" -O abc_0000_step_v00.7z
7z x abc_0000_step_v00.7z -ostep/ -y

# Extract XT files from parasolid zips
for id in 00000000 00000001 00000005 00000007 00000008 00000009; do
    unzip -o -q ~/cadatomic/xt-parser/test-data/abc/extracted/$id/*.zip -d /tmp/abc_validate/$id/
done
```

### Cross-Validation Commands

```sh
# STEP ground truth (topology counts)
grep -c "ADVANCED_FACE" file.step     # → face count
grep -c "EDGE_CURVE" file.step        # → edge count
grep -c "VERTEX_POINT" file.step      # → vertex count

# xt-winnow output
cargo run --release --example parse_xt -- file.x_t
# Output: body[0]: type=Solid, shells=N, surfaces=N, curves=N, edges=N, vertices=N

# Batch validation
cargo run --release --example validate -- ~/cadatomic/xt-parser/test-data/abc/xt_files/

# Euler characteristic check: V - E + F ≈ 2 × shells (for closed solids)
```

### Current Status (6 ABC models, multi-file)

| Model | STEP Faces | XT Faces | STEP Edges | XT Edges | Status |
|-------|:--:|:--:|:--:|:--:|:--|
| 00000000 | 25 | 25 | 33 | 33 | match (6 files) |
| 00000001 | 103 | 103 | 246 | 246 | match (9 files) |
| 00000005 | 60 | 60 | 120 | 120 | match (5 files) |
| 00000007 | 3 | 3 | 2 | 2 | match |
| 00000008 | 21 | 21 | 34 | 34 | match |
| 00000009 | 6 | 6 | 5 | 5 | match |

6/6 perfect match. 100 ABC models: 90/100 parse OK, 89/100 STEP face/edge match. Remaining 10 parse failures from digit concatenation across X_T line breaks (column-80 wrapping). 4 "OK but mismatched" models have XT > STEP face counts (extra sheet/wire bodies in multi-file XT).

---

## Debug/Fix Loop

When a file fails to parse or produces wrong topology counts:

### Step 1 — Identify the failing entity type

```sh
cargo run --example parse_xt -- file.x_t 2>&1 | grep "stopped"
# → [xt-winnow] stopped at type_id=N after M entities: <error>
```

### Step 2 — Find the raw data in the stream

The XT body is a flat token stream (newlines are whitespace). To find a specific entity:
```python
with open('file.x_t') as f: text = f.read()
body = text.split('**END_OF_HEADER')[1].split('\n', 1)[1].replace('\n', ' ')
idx = body.find(' <type_id> ')
print(body[idx:idx+200])
```

### Step 3 — Check the schema definition in sch_13006.s_t

```sh
grep -A20 "^<type_id> " ~/cadatomic/solidworks/SOLIDWORKS/data/pschema/sch_13006.s_t
```

Count only `transmit_flag=1` fields. Expand multi-element fields (`extra2>1`). Check if last field has `extra2=1` (variable-length).

### Step 4 — Verify with Ghidra decompilation

```sh
ghidra decompile 0x180a24ab0  # pk_receive_entity_typed — main read loop
ghidra decompile 0x180a1dbe0  # pk_read_field_data — field type dispatch
```

Look at how pskernel.dll actually reads this entity type. Compare field count and types against your schema.rs entry.

### Step 5 — Fix schema.rs

Common fixes:
- **Type not in sch_13006**: Remove from `ps13_schema()` so it falls through to Path B (inline schema from file)
- **Multi-element field**: Use `P2`, `P3`, `F2`, `F3`, `C2` FieldType variants
- **Wrong field count**: Recount transmitted fields, expand multi-element fields
- **Variable tail**: Set `is_variable=true`, choose correct `VarType`
- **PS30 adds a field**: Hardcode the extra field (like ATTRIBUTE's 9th field)

### Step 6 — Re-run cross-validation

```sh
cargo test && cargo run --release --example validate -- ~/cadatomic/xt-parser/test-data/abc/xt_files/
```

Compare face/edge/vertex counts against STEP. Check Euler characteristic.

### Step 7 — If build counts are wrong but parse succeeds

The issue is in `build.rs`, not parsing. Check:
- Does build.rs follow the correct pointer chain for this entity type?
- Are field indices correct? (PS30 annotation diffs can shift indices for BODY)
- Multi-body files: face chain may start from a different shell than the first BODY

---

## Reference Material

### Parasolid Reference Manual

Located at `~/cadatomic/solidworks/notes/xt/reference/` (120+ chapters of the Parasolid Functional Description). Key chapters:

| Chapter | Topic | Use When |
|---------|-------|----------|
| ch014 | Model structure | Understanding entity relationships (body→shell→face→loop→fin→edge) |
| ch015 | Body types | Solid vs sheet vs wire body differences |
| ch016 | Session/local precision | Resolution values (res_size, res_linear) |
| ch017 | Geometry | Parameterizations for all curve/surface types |
| ch019 | Nominal geometry | Approximate geometry handling |
| ch020 | Transformations | TRANSFORM entity format |
| ch021 | Assemblies/instances | ASSEMBLY and INSTANCE entities |
| ch092 | Attribute definitions | ATTRIB_DEF structure |
| ch093 | Attributes | ATTRIBUTE entity and chains |
| ch098 | Archives | Transmit format details (read/write pipeline) |
| ch121 | Math form of B-geometry | NURBS curve/surface mathematical definitions |
| ch124 | Glossary | Parasolid terminology |

### Reverse Engineering Notes

Located at `~/cadatomic/solidworks/notes/xt/`:

| File | Content |
|------|---------|
| `schema_preamble_text_parser.md` | Definitive T-line + annotation char format (from Ghidra) |
| `entity_loop_dispatch.md` | Entity read sequence from Ghidra decompilation |
| `entity_field_reader.md` | Per-field-type read functions (addresses + behavior) |
| `annotated_receive_path.md` | Full PK_PART_receive pipeline annotated |
| `annotated_write_path.md` | Write ordering (useful for understanding field order) |

### Schema Files

Located at `~/cadatomic/solidworks/SOLIDWORKS/data/pschema/`:

| File | Version | Notes |
|------|---------|-------|
| `sch_13006.s_t` | PS 13.0 base | Base schema for ABC dataset files |
| `sch_30100.s_t` | PS 30.1 | Mostly identical to 13006 for core types |
| `sch_37102.s_t` | PS 37.1 | Latest schema available |

### ABC Dataset

Located at `~/cadatomic/xt-parser/test-data/abc/`:

| Path | Content |
|------|---------|
| `xt_files/` | Extracted .x_t files (simple CAD models) |
| `extracted/<id>/*.zip` | Parasolid zip archives by model ID |
| `step/` | STEP files for cross-validation (download from NYU) |
| `ofs/` | FeatureScript YAML (parametric construction history) |

---

## Known Issues

1. **ATTRIBUTE is variable-length** — sch_13006: 7 fixed fields + variable pointer array (VERSION int = array count). PS30 files typically have version=1 (1 pointer).
2. **ATTRIB_DEF.callbacks transmitted despite transmit=0** — PS30 transmits this field. Hardcoded in schema.
7. **Digit concatenation at line breaks** — X_T column-80 wrapping can split numbers across lines. After newline stripping, `3\n1` becomes `31`. This is intentional for long floats (17-digit) but causes desync when integer tokens happen to fall at column 80. Affects ~10% of Onshape ABC models. Fix requires implementing Parasolid's exact trailing-space-before-newline stripping logic (see entity_field_reader.md §2.2).
3. **build.rs BODY field indices** — PS30 annotated BODY has 34 fields; geometry chain pointers at [19,20,21] (surf/curve/point), shell at [18], body_type at [14], region at [24]. PS13 base (23 fields) uses [3,4,5] for geometry, [16] for shell.
5. **Vertex under-counting** — some XT files omit VERTEX/POINT entities entirely. STEP infers vertices from edge endpoints; XT doesn't always serialize them.
6. **`?` notation** — Behavior depends on field type:
   - **Pointer (`p`)**: `?N` = optional pointer to entity N. `?` consumed, integer N read normally.
   - **Float (`f`)**: `?` = NaN sentinel. Only `?` consumed (1 byte), following digits belong to NEXT field.
   - **Vector (`v`)**: `?` = entire vector is NaN (all 3 components). Only `?` consumed; NOT 3 separate `?` reads.
   - **Box (`b`)**: `?` = all 6 components NaN. Single `?` consumed.
   - **Interval (`i`)**: `?` = both bounds NaN. Single `?` consumed.
   - Confirmed from Ghidra RE: text_read_vector (0x182055440) checks `?` once at start, fills all components.
