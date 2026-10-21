# `parse_embedded_binary_archive_37102` rejects every real SOLIDWORKS deltas member

- Filed: 2026-10-21, by an RE subagent of the solidworks-re session (`main`), with the user's approval to file
  xt-parser bug reports under `docs/bugs/`.
- Commit tested: `e506932df7dffc35410a9a1113bc1b78513051d5` (main). Built from `git archive e506932` in a scratch
  directory; library code unchanged.
- Evidence and tools: solidworks-re
  `artifacts/port/S2/20261021T110213Z-deltas-decode/` (notes.md §2–§4, `tools/`). The input members stay in that
  repo's ignored `raw/`. This report holds no SOLIDWORKS-derived bytes.

## Command

The test uses an extra bin that only calls the public API:

- source: solidworks-re `artifacts/port/S2/20261021T110213Z-deltas-decode/tools/xt_dump_archive.rs`;
- `archive P D` calls `xt_schema::parse_embedded_binary_archive_37102(&P, &D)`;
- `member M` calls `xt_parser::EmbeddedBinaryXtDocument::parse(&M, &sch_13006::SCHEMA, &sch_37102::SCHEMA)`.

```
xt_dump_archive archive <partition member m0> <deltas member m1>
```

The members are the two zlib-chunked records of `Contents/Config-0-Partition` in a SOLIDWORKS 34.1.1 `.SLDPRT`. Both
are headerless neutral binary (`PS\0\0`, `SCH_3701243_37102_13006`, or `SCH_3701229_37102_13006` for the 4 corpus
parts).

## Result

- **Partition / deltas pairs: 0 of 153 parse** (153 documents, 21 unique deltas members).
- In `member` mode, partition m0 parses in 152 of 152 unique members, and GhostPartition m0 in 153 of 153. So the failure
  is in the deltas member, after the new continuation-index check has passed.

| example document | doc sha256 | partition m0 sha256 | deltas m1 sha256 | error | docs with this error |
|---|---|---|---|---|---|
| block seed | 9a3cf496b87a4f68d9740aba2c3bf4f426e24d12cfd7a91cfd750cb497315074 | 1a41d17933144724a8bd6b8c26fd85a47ca79373111d4fe677a5c975eaeadba0 (6496 B) | 623190dfef33f48638862b9c83771b2e9ac8df81c13dbfdc5bdc29276c9bd417 (1112 B) | `TrailingData` @148: "964 logical bytes remain after the terminator" | 140 |
| fillet seed | 6125f5fe6ac774e15fb57ead3db91e1349ffbcc80d00404189a4f751794de5c2 | 4e0db7140542cebb3de77e8122710a2726f2285881ccbd8ef086e4e5576f0430 (7467 B) | 9fefb864d03d614b66aa0af7a3f2886f4fc57b78f10ba11ae84d73dc92fd4445 (5107 B) | `InvalidNode` @201..203: "node type 256 is absent from target schema" | 7 |
| block, Boss suppressed | 71360693cf912a2b9466f306495d32ba6d92d3b079811913ad0e177aa149ccd4 | 997b2ecd3831b622cc622ad417427f950f28976608693bfe7313a9a2294cff9f (307 B) | 92dafd7a94196b5ae8a16d726d6724762dd836e9bacc9e995846c594f25a292a (411 B) | `TrailingData` @148: "263 logical bytes remain after the terminator" | 2 |
| corpus part | 53f21949647efac2a5e8501561e94520509b0a704f2652dcda9215959d4ca576 | 4e81cbe78f8043c9b2d4c6d612db12612c0568c557e98ee32448eda1a2024774 | 1fba94d5825b97d459e7bcb7dad72e51d7501096abaa8f505d979ff56d395b8b (515677 B) | `InvalidNode` @175..179: "terminator index must be 0, got 374" | 1 |
| corpus part | 8a9bba8166fbfce68aee7f95fab5bd6c0623cdbc25e2009e17ca38495ccbf497 | 6d3489d571a64116dc713948736f8bc8f30faa8f5a9b41a4e6f8c8f7234a27e9 | 17937071b52ea23346fb83f3047e37965b3cd37d7b4a61b14f1e306488f4f382 (15123172 B) | `InvalidNode` @185..187: "node type 256 is absent from target schema" | 1 |
| corpus part | abd9fa445c8a054b57d2537fe95cc0939b8b2c23dafd38fb35e337d57a9f84fc | 81a26eee8ba45789296cc747d120fc6b854db8c5c828382387eaed75ba893c80 | 4cf774eddda1d623d51fed672c6253850166c4325a87e80cd3001f8694776d7f (1244886 B) | `InvalidNode` @198..200: "node type 257 is absent from target schema" | 1 |
| corpus part | f161f52cc864e0899b636f8664e5d01c37af68cecb9001ec36eb6c569a0b74ff | 655784c39f24899d83d6314fbd0805ca368e40317ea625ee6b0685f5e74d6e84 | 268093d633cfc572b1cbc7ff521038010c2c664654cb94e01762c095b69d4494 (8168455 B) | `InvalidNode` @175..177: "node type 473 is absent from target schema" | 1 |

## Cause

`EmbeddedBinaryXtArchive` assumes a deltas member is one node graph whose indices continue the partition's and never
repeat. Byte reading of the 21 unique members shows a different grammar (`static-observed`):

```
member  := prefix unit+ final
unit    := graph_A TERM  list  i32 n  graph_B TERM
graph_A := [PARTITION] PMARK+ recordA*   PARTITION only in the first unit; the unit's delta belongs to its last PMARK
list    := (index: pointer, node_type: i16)*  then index 0 (the ordinary "00 01" encoding)
graph_B := recordB*
final   := PMARK* TERM, then end of input   (PMARKs without a predecessor; empty in 20 of 21 members)
TERM    := the ordinary terminator record (node type 1, index 0)
```

1. **Pointer width.** PARTITION and PMARK records use the ordinary 2-byte pointer index. Every other record in a deltas
   member (recordA, recordB) encodes each pointer as the ordinary index followed by a u8 flag (0 or 1). Reading those
   records with 2-byte pointers is what produces "node type 256/257/473" and the bad terminator index.
   - The flag is a pure function of the target node type over 2,608,937 pointers in 20 members: 0 mixed types,
     0 mixed pointer classes.
   - Flag 0 marks types without a node_id: INT/REAL/CHAR/UNICODE_VALUES, GEOMETRIC_OWNER, NURBS_CURVE/SURF,
     BSPLINE_VERTICES, KNOT_SET/MULT, CURVE_DATA, SURFACE_DATA, POINTER_LIS_BLOCK, CHART, LIMIT.
   - Flag 1 marks entities, ATTRIBUTE, ATTRIB_DEF, BODY, HALFEDGE, WORLD, and null.
   - Its meaning is a hypothesis.
2. **Several graphs.** The member holds several TERM-closed graphs with (index, type) lists and an i32 between them, so
   "N logical bytes remain after the terminator" fires after the first graph.
3. **Continuation rule.** It holds for the new records: the first deltas record (PARTITION) has index = partition max + 1
   in 21 of 21 members, and the PARTITION, PMARK and recordA indices form one contiguous block from there. They are
   **not written in ascending order** (block: 176, 177, 179, 178).
4. **Repeated indices are normal.** Every recordB index and every list index is an existing index, in the partition or
   new earlier in the deltas member. recordB records are pre-images that replace those nodes, so the "may not repeat
   indices" rule is wrong for this format.

Semantics (`inferred`):

- Every PMARK has `delta_is_forward = false`, and the pmark chain is linear.
- Each unit is the backward delta of its owning pmark:
  - delete each list entry (its type must match the live node);
  - insert each recordA at its new index;
  - replace each recordB at its existing index (same type).
- Applying the units from `PARTITION.current_pmark` along `preceding` replays 152 of 152 decodable pairs with 0 type
  errors and 0 dangling pointers at every step (26.7 M pointer checks). Examples:
  - the fillet part rolls back to the pre-fillet box;
  - the 4 corpus parts unwind feature by feature to an empty world.

## Expected behaviour

- `parse_embedded_binary_archive_37102` should parse these members per the grammar above. It should expose units,
  lists, the i32 and the two record graphs, and record the pointer flag. Recording the flag rather than validating it
  keeps the flag rule a hypothesis.
- It should resolve pointers over the partition plus the new deltas indices, and accept recordB indices that repeat
  existing ones.
- At minimum it should not report the archive as unparseable when only the deltas grammar is unsupported.
- The partition-only path (`EmbeddedBinaryXtDocument::parse`) already works on every real member tested.

## Evidence grade

- `static-observed` (bytes):
  - The grammar consumes 21 of 21 unique deltas members exactly.
  - The independent Python walker used for it matches xt-parser e506932 node for node, and field for field, on 152 of
    152 partition members.
- `inferred`: the delta semantics (replay above). The alternatives are named in solidworks-re notes.md §4.
- `hypothesis`: the meaning of the pointer flag, and of the i32 `n`. `n` equals the recordB count in 74 of 124 units;
  otherwise n − recordB is 1 (24 units), 2 (6), 3 (19) or 8 (1).
