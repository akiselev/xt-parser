# Generated Rust API

The generated code is the deployable replacement for loading `.s_t` files at
runtime. It retains the complete parser specification while keeping semantic
Rust types separate from raw metadata.

## Module layout

Each supported schema has the same generated hierarchy:

```text
xt_schema::schemas::sch_37102
├── metadata       source hashes, schema identity, inventory counts
├── spec           complete parser-facing Schema, NodeSpec, and FieldSpec data
├── pointers
│   ├── exact      exact node targets such as BodyNode
│   └── classes    abstract targets such as SurfaceClass
├── hierarchy      inferred family traits and auditable FAMILIES metadata
└── nodes
    ├── families
    │   ├── class_1006_surface
    │   ├── class_1008_curve
    │   ├── class_1019_attributes_features
    │   └── class_1043_node_map
    ├── standalone
    └── Node        enum over every transmitted record type
```

The concrete family modules vary by schema because they are inferred from that
schema. All record structs are re-exported through `nodes`, so callers can use
`schemas::sch_37102::nodes::Body` without knowing its storage module.

## Automatic hierarchy inference

No node ID, node name, class name, or semantic group is configured by hand.
For every schema:

1. Collect abstract pointer classes (`pointer_class >= 1000`).
2. For each class, collect nodes with a `next` or `previous` pointer to it.
3. Retain classes with at least two member nodes as linked families.
4. Infer the label from the most frequent non-link field name pointing to the
   class, breaking ties deterministically.
5. Emit a family module, marker trait, pointer alias, member list, and the field
   that supplied the label.
6. If a node belongs to multiple inferred families, implement every family
   trait and choose the lowest class number as its deterministic storage module.

This is an `inferred` organization of documented schema relationships, not a
claim about Parasolid's private C++ inheritance. The generated `FAMILIES`
constant makes every decision inspectable.

For example:

```rust
use xt_schema::schemas::sch_37102::{
    hierarchy::{FAMILIES, SurfaceFamily},
    metadata::METADATA,
    nodes::{Body, Plane},
    pointers::{BodyNode, SurfaceClass},
};

fn requires_surface<T: SurfaceFamily>() {}
requires_surface::<Plane>();

assert_eq!(METADATA.schema_key, "SCH_37102");
assert_eq!(BodyNode::NODE_TYPE, 12);
assert!(FAMILIES.iter().any(|family| family.label == Some("surface")));
```

`Body` is imported to illustrate the stable `nodes` surface; exact and abstract
pointer aliases remain type-level tags on `XtPointer<T>`, not Rust references.

## Regeneration

```bash
PARASOLID_SCHEMA_DIR="/path/to/pschema" scripts/generate-schemas.sh
scripts/generate-schemas.sh --check
```

The generator replaces only a matching schema directory whose existing files
all carry its generated-file banner. This both removes stale generated modules
and prevents an accidental output path from deleting handwritten source.

## Resolved B-rep view

`xt_schema::view::sch_30100` (and `sch_37102`) index a typed document by file
index and expose a read-only topology walk:

`Model::from_text(&TypedTextDocument)` or `Model::from_binary(..)`, then
`Model::bodies()` -> `Body` -> `regions()` -> `Region` (`kind()`, `shells()`) ->
`Shell` (`faces()` = faces whose back shell it is, `front_faces()`,
`wire_edges()`, `isolated_vertices()`) -> `Face` (`sense()`, `back_shell()`,
`front_shell()`, `loops()`, `surface()`, `agrees_with_surface()`, `tolerance()`)
-> `Loop::boundary()` = `LoopBoundary::Circuit(Vec<Fin>)` or
`IsolatedVertex(Vertex)` -> `Fin` (`sense()`, `edge()`, `vertex()`, `next()`,
`other()`, `curve()`) -> `Edge` (`curve()`, `tolerance()`, `fins()`) ->
`Vertex` (`position()`, `tolerance()`).

`Surface` is `Plane | Cylinder | Cone | Sphere | Torus` with plain values, or
`Other(OtherSurface { kind, sense, node })`; `Curve` is `Line | Circle | Ellipse`
or `Other(OtherCurve { kind, sense, node })`. An unset tolerance is `None` and
means exact. Every failure is a `ViewError` naming the field; the walk never
guesses.

`xt_schema::parse_text_embedded` and `parse_binary_embedded` decode raw records
for any version whose embedded schema edits apply to the V13 base, using each
file's own layouts; typed conversion remains limited to the generated schemas.
