# Parasolid oracle

The oracle is an out-of-process executable in
`/projects/sinbad/parasolid-rs/crates/parasolid-test/src/bin/xt-oracle.rs`.
It stages an arbitrary text XT file, calls documented public PK receive/query
APIs, and emits versioned JSON.

It fingerprints:

- official receiver acceptance or rejection stage;
- runtime kernel version and caller-supplied DLL SHA-256;
- input SHA-256 and size;
- body type and B-rep topology counts;
- axis-aligned bounds; and
- amount/mass, center of gravity, inertia, and periphery.

Build and run under the existing Wine environment:

```bash
cargo build \
  --manifest-path /projects/sinbad/parasolid-rs/Cargo.toml \
  -p parasolid-test --bin xt-oracle --target x86_64-pc-windows-gnu

fixture_windows=$(winepath -w /absolute/path/to/model.x_t)
wine /projects/sinbad/parasolid-rs/target/x86_64-pc-windows-gnu/debug/xt-oracle.exe \
  "$fixture_windows" \
  --dll-sha256 c900fa3f430fe67c2adb15a50f9604d9812ed81a02c5006b66689dac28263073 \
  --parasolid-rs-commit "$(git -C /projects/sinbad/parasolid-rs rev-parse HEAD)" \
  --source-state dirty
```

Use `--source-state clean` only after confirming
`git -C /projects/sinbad/parasolid-rs status --porcelain` is empty. Dirty
oracle runs are valid experiments but require the patch or a later commit before
they are independently reproducible.

The first live probe, against ABCD file SHA-256
`fd5934c3f05be19f163aae1f5e0cfced44d8d29c5f0e33519ce95c41bc45112a`,
was accepted by V37.01.243 as one solid with 23 faces, 50 edges, 32 vertices,
and amount `0.1422288445389974`. This is `dynamic-observed`; it does not yet
validate every native field interpretation.

For that same file, native parsing and generated conversion produced 616 nodes.
The graph counts for regions (2), shells (2), faces (23), loops (28), edges
(50), and vertices (32) exactly match the public-PK oracle. This is a strong
topology cross-check, but geometry values still need broader per-field and
one-factor comparisons.

## File-by-file cross-check (`oracle/`)

`oracle/` is a separate Cargo workspace so the native parser never needs Wine,
MinGW or the Parasolid DLL. It describes each file twice and compares the two
descriptions:

- `xt-native-dump` (host): parses with `xt-schema`, walks the resolved view
  (`xt_schema::view`) and writes one JSON `FileDigest` per file.
- `xt-pk-dump` (`x86_64-pc-windows-gnu`, run under Wine): receives the same file
  into Parasolid with `parasolid::fileio::receive`, queries faces, loops, fins,
  edges, vertices, surfaces, curves, precisions and the body box through
  `parasolid-rs`, and writes the same digest. It appends and flushes after every
  file so a crashed run can resume.
- `xt-xcheck-compare` (host): compares digest pairs and writes a TSV plus a
  summary.
- `oracle/run.sh LIST RUN_DIR` runs all three. A file that kills the Parasolid
  process is recorded as an oracle failure at stage `crash`.

Run the Wine side only through `cadabra3/scripts/cargo-locked`; `run.sh` does.

What is compared per body (tolerance: `|a-b| <= 1e-9 * (1 + max(|a|,|b|))`;
entity tolerances use the relative form `1e-9 * max(|a|,|b|)`):

| Category | Native | Parasolid |
|---|---|---|
| `counts` | regions, shells, faces, loops, edges, vertices, fins from the view | `PK_BODY_ask_*`, loop fins |
| `surface_classes`, `curve_classes` | multiset of carrier classes | `PK_SURF_ask_class` / curve type mapped to the same classes |
| `surface_params`, `curve_params` | plane origin and normal; cylinder/cone/torus origin, axis, radii, cone half angle; sphere centre and radius; line, circle, ellipse | `PK_*_ask` |
| `vertex_positions` | vertex points | `PK_VERTEX_ask_point` |
| `tolerances` | edge and vertex tolerance, `None` for unset (exact) | `PK_EDGE/VERTEX_ask_precision`; a value of at most half the session precision is exact |
| `bounds` | box of the vertices | `PK_TOPOL_find_box`; the native box must lie inside it |
| `face_sense` | face sense combined with surface sense | `PK_FACE_ask_oriented_surf` |
| `face_loops`, `edge_fins` | loops per face; fins per edge | same queries |

Entities are paired by node identifier (the XT `node_id`, Parasolid's entity
identifier). When the identifier sets differ, zero or repeat, pairing falls back
to the sorted geometry. An oracle that cannot load or query a file is
`oracle_silent`, never a pass.

Facts the comparison established, `dynamic-observed`:

- Parasolid presents a trimmed-curve wrapper as its basis curve; the view does
  the same in `Edge::curve` and `Fin::curve`.
- Parasolid's face orientation is the face sense combined with the surface
  sense.
- A sheet edge has a loop-less partner fin in its `other` ring; Parasolid counts
  only fins that belong to a loop.
- Wire edges are on the shell's edge list, not the body's; an isolated vertex is
  on the shell's vertex list.
- A loop whose only halfedge has no edge is an isolated vertex loop with no fins.
- Parasolid reports an exact edge or vertex with half the session precision.

`xt-embedded-check` (also in `oracle/`) decodes with embedded-schema selection
and with the exact schema and requires identical records.
`xt-pk-reencode` receives files and transmits them at chosen schema versions;
only the current version works without schema files (see STATUS.md).
