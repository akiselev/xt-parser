# Evidence policy

Material claims use exactly one grade:

- `documented`
- `export-observed`
- `static-observed`
- `dynamic-observed`
- `inferred`
- `hypothesis`

Each promoted claim records the source hash, source location or function
address, command or public-PK probe, artifact path, result, alternatives, and a
minimal regression test.

The old `xt-parser`, `xt-parser2`, `parasolid-re/docs/XT_FORMAT.md`, and historic
XT notes are quarantined hypotheses. They may suggest experiments but cannot
promote a claim.

The current static anchors for focused receiver work include:

| Function | DLL address | Grade |
|---|---:|---|
| `DS_receive_header` | `0x180a1b8e0` | `static-observed` |
| `DS_receive_start` | `0x180a1bdd0` | `static-observed` |
| `DS_read_schema_header` | `0x180a186f0` | `static-observed` |
| `read_from_schema_file` | `0x180a19510` | `static-observed` |
| `read_node` | `0x180a24ab0` | `static-observed` |
| `DS__open_xmt_txt` | `0x183201dd0` | `static-observed` |
| `DS__ip_char_txt` | `0x1820540c0` | `static-observed` |
| `DS__ip_double_txt` | `0x182054480` | `static-observed` |
| `DS__ip_index_txt` | `0x182054680` | `static-observed` |
| `DS__ip_int_txt` | `0x182054990` | `static-observed` |
| `DS__ip_logical_txt` | `0x182054fa0` | `static-observed` |

These names and addresses are navigation anchors, not permission to copy
decompiled implementation or call internal functions.
