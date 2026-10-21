//! Parasolid-written bodies: the view must reproduce the kernel's own counts.
//!
//! Fixtures are vendored under `tests/fixtures` (see `SOURCE.txt`). `pk-unusual`
//! files are default embedded-form text; `pk-minimal` files are the bare,
//! non-embedded `SCH_3701243_37102` stream.

use std::{fs, path::PathBuf};

use serde_json::Value;
use xt_schema::{
    TextDocument,
    view::{
        BodyKind, RegionKind,
        sch_37102::{LoopBoundary, Model},
    },
};

fn fixtures(dir: &str) -> Vec<(String, Vec<u8>, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut out = Vec::new();
    for entry in fs::read_dir(&root).expect("fixture directory") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "x_t") {
            let bytes = fs::read(&path).expect("read x_t");
            let json = fs::read_to_string(path.with_extension("json")).expect("read json");
            let name = path
                .file_stem()
                .expect("stem")
                .to_string_lossy()
                .into_owned();
            out.push((name, bytes, serde_json::from_str(&json).expect("json")));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Counts of one body as the view sees them.
#[derive(Debug, Default, PartialEq)]
struct Seen {
    kind: Option<BodyKind>,
    region_kinds: Vec<&'static str>,
    shells: usize,
    faces: usize,
    loops: usize,
    fins: usize,
    edges: usize,
    vertices: usize,
    fins_per_edge: Vec<usize>,
    acorn_vertices: usize,
    wire_edge_uses: usize,
}

fn see(document: &TextDocument) -> Vec<Seen> {
    let TextDocument::Sch37102(document) = document else {
        panic!("fixtures are SCH_37102");
    };
    let model = Model::from_text(document).expect("model");
    model
        .bodies()
        .map(|body| {
            let mut seen = Seen {
                kind: Some(body.kind().expect("kind")),
                ..Seen::default()
            };
            for region in body.regions().expect("regions") {
                seen.region_kinds
                    .push(match region.kind().expect("region kind") {
                        RegionKind::Void => "void",
                        RegionKind::Solid => "solid",
                    });
                for shell in region.shells().expect("shells") {
                    seen.shells += 1;
                    seen.acorn_vertices +=
                        usize::from(shell.acorn_vertex().expect("acorn").is_some());
                    seen.wire_edge_uses += shell.wire_edges().expect("wire").len();
                }
            }
            let faces = body.faces().expect("faces");
            seen.faces = faces.len();
            for face in faces {
                let loops = face.loops().expect("loops");
                seen.loops += loops.len();
                for l in loops {
                    if let LoopBoundary::Circuit(circuit) = l.boundary().expect("boundary") {
                        seen.fins += circuit.len();
                    }
                }
            }
            let edges = body.edges().expect("edges");
            seen.edges = edges.len();
            seen.fins_per_edge = edges
                .iter()
                .map(|e| e.fins().expect("fins").len())
                .collect();
            seen.fins_per_edge.sort_unstable();
            seen.vertices = body.vertices().expect("vertices").len();
            seen.region_kinds.sort_unstable();
            seen
        })
        .collect()
}

fn stored_kind(pk_name: &str) -> BodyKind {
    match pk_name {
        "PK_BODY_type_solid_c" => BodyKind::Solid,
        "PK_BODY_type_sheet_c" => BodyKind::Sheet,
        "PK_BODY_type_wire_c" | "PK_BODY_type_acorn_c" | "PK_BODY_type_minimum_c" => BodyKind::Wire,
        "PK_BODY_type_general_c" | "PK_BODY_type_empty_c" => BodyKind::General,
        other => panic!("unexpected body type {other}"),
    }
}

fn count(value: &Value, key: &str) -> usize {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing {key}")) as usize
}

#[test]
fn unusual_bodies_match_the_kernels_counts() {
    let fixtures = fixtures("pk-unusual");
    assert_eq!(fixtures.len(), 39);
    for (name, bytes, json) in fixtures {
        let document = xt_schema::parse_text(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let seen = see(&document);
        let expected = json["result"]["bodies"].as_array().expect("bodies");
        assert_eq!(seen.len(), expected.len(), "{name}: body count");
        for (seen, expected) in seen.iter().zip(expected) {
            let c = &expected["counts"];
            let context = &name;
            assert_eq!(
                seen.kind,
                Some(stored_kind(c["body_type"].as_str().expect("type"))),
                "{context}: body type"
            );
            let mut kinds: Vec<_> = c["region_kinds"]
                .as_array()
                .expect("kinds")
                .iter()
                .map(|k| k.as_str().expect("k"))
                .collect();
            kinds.sort_unstable();
            assert_eq!(seen.region_kinds, kinds, "{context}: region kinds");
            let shells: u64 = c["shells_per_region"]
                .as_array()
                .expect("shells")
                .iter()
                .map(|n| n.as_u64().expect("n"))
                .sum();
            assert_eq!(seen.shells as u64, shells, "{context}: shells");
            assert_eq!(seen.faces, count(c, "faces"), "{context}: faces");
            assert_eq!(seen.loops, count(c, "loops"), "{context}: loops");
            assert_eq!(seen.fins, count(c, "fins"), "{context}: fins");
            assert_eq!(seen.edges, count(c, "edges"), "{context}: edges");
            assert_eq!(seen.vertices, count(c, "vertices"), "{context}: vertices");
            let mut per_edge: Vec<usize> = c["fins_per_edge"]
                .as_array()
                .expect("fpe")
                .iter()
                .map(|n| n.as_u64().expect("n") as usize)
                .collect();
            per_edge.sort_unstable();
            assert_eq!(seen.fins_per_edge, per_edge, "{context}: fins per edge");
            assert_eq!(
                seen.acorn_vertices,
                count(c, "acorn_vertices"),
                "{context}: acorn vertices"
            );
            assert_eq!(
                seen.wire_edge_uses,
                count(c, "wireframe_edge_uses"),
                "{context}: wireframe edges"
            );
        }
    }
}

#[test]
fn minimal_non_embedded_streams_parse_and_match_the_kernels_counts() {
    let fixtures = fixtures("pk-minimal");
    assert_eq!(fixtures.len(), 5);
    for (name, bytes, json) in fixtures {
        assert!(bytes.starts_with(b"T51 "), "{name}: bare kernel stream");
        let document = xt_schema::parse_text(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            document.raw().envelope.header.is_none(),
            "{name}: headerless"
        );
        let seen = see(&document);
        let expected = json["bodies"].as_array().expect("bodies");
        assert_eq!(seen.len(), expected.len(), "{name}: body count");
        for (seen, expected) in seen.iter().zip(expected) {
            assert_eq!(seen.kind, Some(BodyKind::Solid), "{name}");
            assert_eq!(expected["body_type"], "Solid", "{name}");
            assert_eq!(seen.faces, count(expected, "faces"), "{name}: faces");
            assert_eq!(seen.edges, count(expected, "edges"), "{name}: edges");
            assert_eq!(
                seen.vertices,
                count(expected, "vertices"),
                "{name}: vertices"
            );
        }
    }
}

#[test]
fn tolerant_edges_have_no_3d_curve_and_vertex_tolerance_covers_edges() {
    let (_, bytes, _) = fixtures("pk-minimal")
        .into_iter()
        .find(|(name, ..)| name == "r49-xtmin-tolblock")
        .expect("tolblock fixture");
    let TextDocument::Sch37102(document) = xt_schema::parse_text(&bytes).expect("parse") else {
        panic!("SCH_37102 expected");
    };
    let model = Model::from_text(&document).expect("model");
    let body = model.bodies().next().expect("body");
    let mut tolerant = 0;
    for edge in body.edges().expect("edges") {
        if let Some(tolerance) = edge.tolerance() {
            tolerant += 1;
            assert!(tolerance > 0.0);
            assert!(
                edge.curve().expect("curve").is_none(),
                "tolerant edge keeps no 3-D curve"
            );
            for fin in edge.fins().expect("fins") {
                assert!(
                    fin.curve().expect("fin curve").is_some(),
                    "tolerant edge fins carry curves"
                );
            }
        }
    }
    assert!(tolerant > 0);
}

#[test]
fn embedded_selection_agrees_with_the_exact_parse_on_unusual_bodies() {
    for (name, bytes, _) in fixtures("pk-unusual") {
        let exact = xt_schema::parse_text(&bytes).expect("exact");
        let embedded =
            xt_schema::parse_text_embedded(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(exact.raw().nodes, embedded.nodes, "{name}");
    }
}
