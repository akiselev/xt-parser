# xt-parser TODO

## Current Status

- **100 ABC models tested**: 90% parse OK, 86% STEP face/edge match
- **6 reference models**: 6/6 perfect STEP match
- **10 parse failures**: all stream desync from unhandled entity/field types
- **4 clean mismatches**: XT has MORE faces than STEP (multi-file XT includes extra sheet/wire bodies not in single-file STEP)

## High Priority

### Fix remaining 10 parse failures (stream desync)

All failures follow the same pattern: bogus type_ids (2848, 705, 817, etc.) after N entities. A previous entity consumed wrong number of tokens, cascading into desync.

**Approach**: Add last-entity tracing, identify the entity type right before each desync, verify field count against sch_13006.

**Likely culprits**:
- **`h`-type handle fields** (vector + 4 doubles + vector + double = 88 bytes). Current implementation reads a fixed layout but the `has_extra` flag (from Ghidra RE at entity_field_reader.md §2.8) controls whether the 4 inner doubles + second vector + final double are read or filled with NaN. We may be reading too many or too few components.
- **Unknown entity types > 200** that aren't in sch_13006 and need Path B inline schema. Some may be Onshape-specific extensions not present in SolidWorks Parasolid. The Path B parser should handle these if the inline schema is well-formed, but there may be edge cases.
- **`?` handling in `h`-type fields** — the `has_extra` parameter may interact with the `?` sentinel in ways we haven't accounted for.

**Failing models**: 00000011, 00000012, 00000014, 00000015, 00000020, 00000027, 00000028, 00000041, 00000084, 00000094

### Parse schema preamble properly

`parse_schema_preamble` only reads N_types and partition_count. For robustness, it should consume all preamble data (per-type annotations) so they don't interfere with entity parsing. Currently works because the entity parser's inline schema reading handles the same annotation format lazily, but this is fragile.

## Medium Priority

### STEP cross-validation at scale

We have 10,000 STEP files and 1,000 XT models. Running validation on all 1,000 would give better coverage and find edge cases. Automate with a script that produces a summary table.

### Wire/sheet body support in build.rs

`build_one_body` handles solid bodies well but wire (type=12) and sheet (type=7) bodies may have different topology chains. Some "clean mismatches" (XT > STEP face counts) may be from sheet bodies that STEP doesn't export.

### Partition support

The Z1 record contains `partition_count` (separate from the preamble's `entity_count`). Files with `partition_count > 0` may have per-entity partition indices that we're not reading. Need to verify with Ghidra RE whether partition indices are written after each entity.

## Low Priority

### Geometry extraction accuracy

Validate NURBS control points, knot vectors, and analytic geometry parameters against STEP equivalents. Current build.rs extracts geometry but values haven't been cross-checked beyond face/edge counts.

### Integration with solverang-cad

The xt-kernel-bridge from the monorepo dependency graph connects xt-parser to the geometric kernel. Once parsing is solid, build this bridge.

### Assembly/instance support

Types 10 (ASSEMBLY) and 11 (INSTANCE) with TRANSFORM (type 100) are parsed but not built into the typed IR. Multi-body assemblies with transforms would need this.

## Resolved

- [x] Path B variable-length entity detection (INTERSECTION_DATA type 204)
- [x] BODY PS30+ field indices (34-field annotated layout)
- [x] NURBS_SURF field index mapping (sch_13006 positions)
- [x] Vector/Box/Interval `?` sentinel (single `?` fills all components, confirmed via Ghidra RE)
- [x] Float `?` sentinel (consumes only `?` byte, leaves digits for next field)
- [x] 6/6 ABC reference models match STEP ground truth
