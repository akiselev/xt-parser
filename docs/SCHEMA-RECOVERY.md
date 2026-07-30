# Schema acquisition and generation

Exact `SCH_13006`, `SCH_30100`, and `SCH_37102` inputs were found in the
installed SolidWorks `data/pschema` directory. They are local code-generation
inputs, not runtime dependencies and not repository inputs.

The SolidWorks files have CRLF line endings. After removing only carriage
returns, both are byte-identical to the corresponding public PKToy copies:

| Schema | Local SHA-256 | LF-normalized SHA-256 | Nodes | Fields |
|---|---|---|---:|---:|
| `SCH_13006` | `cc98d0c96c959cc0216cee45e4adb785340ed99af7ecb6129f84633a738846af` | `0dd291ea706fc306f16a78140e05e595e75c85ab63e4077e68941b127fcfdcc3` | 142 | 1,125 |
| `SCH_30100` | `fa7e505b120bf384996060dde4acdb3af2c193efcc64b6fda3813015f8298259` | `18838cb90466615a57c841896d3842044e97e85f2b5e33b62c41222f4e37ece5` | 184 | 1,402 |
| `SCH_37102` | `7345f7094df516530a7a858d145e80caa80dfbdfe75b8bd51dc7dbc1acc6012d` | `c2d06b56144524347834699b33d9dbfe7e6a1b8d3cd125b1a4d68593464f8e5f` | 206 | 1,556 |

This identity, plus an official ABC sample whose headers say `APPL=Onshape`,
supports the inference that a numbered SCH file is kernel-version metadata, not
an application-specific SolidWorks or Onshape dialect.

Regenerate the derived crate with:

```bash
PARASOLID_SCHEMA_DIR="/path/to/pschema" scripts/generate-schemas.sh
scripts/generate-schemas.sh --check
```

The generator rejects malformed headers, inventory-count mismatches, unknown
field codes, invalid node ordering, and invalid cardinalities. The output
retains the source identities and full parser metadata, then adds the typed Rust
surface. Generated source is split by concern and automatically groups linked
abstract pointer classes into node-family modules and traits. Adding a future
schema is a generator invocation plus an explicit module/export decision; it
does not require hand-writing hundreds of structs or a taxonomy table.

`q` (opaque) and `t` (tag) are represented in generated signatures as
`XtUnsupported`. They were not encountered in the 23,093-file ABCD XT corpus.
If a transmitted fixture reaches either encoding, it must first be characterized
with controlled public-API fixtures before the native decoder is extended.
