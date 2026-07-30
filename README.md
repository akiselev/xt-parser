# xt-parser

`xt-parser` is a from-scratch, native Rust implementation of the Parasolid XT
format. It does not link to Parasolid. A separately run `parasolid-rs` tool is
used as a differential semantic oracle during development.

The workspace separates format mechanics from generated schema APIs:

- `xt-parser` parses the common header, text records, typed scalar fields,
  binary machine descriptors, schema-directed text/binary scalar fields,
  embedded-schema edits, node graph, pointers, root, and terminator.
- `xt-schema` is generated Rust containing the complete `SCH_13006` and
  `SCH_30100`/`SCH_37102` metadata plus strongly typed structs and enums for
  transmitted nodes. Consumers do not load or distribute `.s_t` files at
  runtime.
- `xt-schema-gen` is a separate, tested generator crate. It strictly imports a
  schema file and deterministically emits the versioned modules consumed by
  `xt-schema`.

The generated API preserves schema and source hashes, node IDs and names,
ordered field metadata, transmission flags, pointer classes, element counts,
unset values, raw node indices, and typed pointer targets. It isolates source
metadata, parser specifications, pointer targets, inferred node families, and
typed records instead of placing everything in one flat file. The hierarchy is
inferred from schema pointer relationships; it has no hand-maintained node
taxonomy. Unknown `q` and `t` field encodings remain explicit errors until
observed.

```bash
scripts/check.sh

cargo run -p xt-schema --bin xt-schema-inspect -- \
  "/absolute/path/to/model.x_t"

cargo run -p xt-schema --bin xt-binary-inspect -- \
  "/absolute/path/to/model.x_b"

# Maintainers with the private schema inputs can regenerate deterministically.
PARASOLID_SCHEMA_DIR="/path/to/pschema" scripts/generate-schemas.sh
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[docs/BINARY.md](docs/BINARY.md),
[docs/GENERATED-API.md](docs/GENERATED-API.md),
[docs/SCHEMA-RECOVERY.md](docs/SCHEMA-RECOVERY.md),
[docs/VALIDATION.md](docs/VALIDATION.md), [docs/ORACLE.md](docs/ORACLE.md), and
[STATUS.md](STATUS.md).
