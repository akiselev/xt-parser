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
