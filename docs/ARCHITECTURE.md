# Architecture

## Trust boundary

The native crate has no FFI dependency. Raw XT enters as bytes and leaves the
parser as a lossless graph plus typed views. Parasolid is run out of process only
to generate fixtures or compute semantic fingerprints.

The parser layers are:

1. **Physical envelope** — common header, physical-to-logical text byte mapping,
   binary signature/machine descriptor, and format selection.
2. **Typed stream** — schema-directed reads for `u c l n w d p f i v b h` from
   text or binary. This layer retains raw spans and represents unset values
   explicitly.
3. **Schema** — a versioned base schema plus the inline first-node differences
   introduced in V14. No node is decoded without a concrete schema.
4. **Raw graph** — node type, file index, field values, pointer indices, and
   provenance.
5. **Generated API** — a separate `xt-schema` crate containing isolated source
   metadata, parser node specifications, pointer targets, inferred node-family
   traits, typed node structs, and one enum per generated schema.
6. **Resolution** — index uniqueness, pointer resolution, root/terminator rules,
   and graph invariants.
7. **Semantic views** — topology and geometry projections that never discard the
   underlying raw graph.
8. **Writer** — canonical emission and, later, preservation-oriented emission.

There is intentionally no generic lexer between layers 1 and 2. XT numbers have
a trailing space, but character and logical fields do not. Sequences such as
`?10`, `+0`, and `FFFFT2` are only unambiguous when the active schema supplies
the next field type.

The binary cursor shares the node/schema engine with the text cursor. Neutral
binary fixes big-endian/IEEE/ASCII; typed binary carries byte-order, double, and
character descriptors; bare binary requires an explicit caller-supplied machine
description. VAX D-float and EBCDIC descriptors are identified and rejected
until their conversions are independently validated.

## Correctness invariants

- Every decoded value has a raw byte span.
- Physical newline removal remains reversible through the offset map.
- Normal numbers consume exactly one separating space; unset `?`, characters,
  and logicals do not.
- The first node is index 1 and is the root.
- Subsequent nodes may be unordered.
- A text terminator is short value 1 followed by index 0.
- Every node index is unique.
- Every non-null transmitted pointer is either resolved or explicitly reported
  under the documented unresolved-index rule.
- A successful strict parse consumes the logical stream exactly.

## Schema and code-generation strategy

The V14+ embedded data is a difference from V13 `SCH_13006`, not a standalone
schema preamble. It appears at the first transmitted node of each type. Therefore
modern decoding uses:

- the exact, provenance-recorded `SCH_13006` base;
- the exact target schema, currently `SCH_30100`;
- parsing first-occurrence flags and `C/D/I/A/Z` edits;
- constructing the effective field list before reading that node; and
- caching the effective node layout for later occurrences.

The separate `xt-schema-gen` crate parses the private `.s_t` source once and
emits ordinary Rust into `xt-schema`. Each version is a module directory with:

- `metadata.rs` for source hashes, schema identity, and inventory counts;
- `spec.rs` for the complete parser-facing node and field specifications;
- `pointers.rs` for exact-node and abstract-class pointer targets;
- `hierarchy.rs` for inspectable, automatically inferred node families;
- `nodes/families/*.rs` for records assigned to inferred families;
- `nodes/standalone.rs` for transmitted records without family evidence; and
- `nodes/mod.rs` for the schema-wide `Node` enum and checked conversion.

Family inference uses only relationships in the schema. A repeated set of node
types whose `next` or `previous` field targets the same abstract pointer class
forms a family. Its label is the most frequent non-link field name pointing to
that class. There is no node-ID/name classification table. The generated
`FAMILIES` value retains the class, label source, module name, and member IDs so
the inference can be audited or revised.

The parser crate remains schema-independent, while an oracle or application can
depend only on `xt-schema` and never open a schema file. Regeneration is
deterministic and `scripts/generate-schemas.sh --check` detects drift.

The generated pointer type is an index tagged with its declared schema target,
not a Rust reference. This preserves null versus unset and avoids self-
referential ownership. Graph resolution can later provide borrowed semantic
views over those typed indices.
