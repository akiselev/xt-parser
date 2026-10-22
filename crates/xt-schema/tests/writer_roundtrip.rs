//! Parse, write, parse again: the typed nodes must come back unchanged.

use std::{fs, path::PathBuf};

use xt_parser::{XtDouble, XtPointer, XtVector};
use xt_schema::{
    TextDocument, parse_text,
    schemas::sch_37102::{Node, nodes::Body},
    write::{TextWriter, WriteError},
};

fn fixture_files(dir: &str) -> Vec<(String, Vec<u8>)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut out: Vec<_> = fs::read_dir(&root)
        .expect("fixture directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "x_t"))
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, fs::read(&path).expect("read x_t"))
        })
        .collect();
    out.sort();
    out
}

fn nodes_of(bytes: &[u8]) -> Vec<Node> {
    match parse_text(bytes).expect("parse") {
        TextDocument::Sch37102(document) => document.nodes,
        TextDocument::Sch30100(_) => panic!("fixtures are SCH_37102"),
    }
}

fn session_of(nodes: &[Node]) -> Option<(f64, f64)> {
    let Some(Node::Body(Body {
        res_size,
        res_linear,
        ..
    })) = nodes.first()
    else {
        return None;
    };
    Some((res_size.0?, res_linear.0?))
}

fn round_trip(name: &str, bytes: &[u8]) -> Result<(), String> {
    let nodes = nodes_of(bytes);
    let (res_size, res_linear) = session_of(&nodes).ok_or_else(|| {
        format!(
            "{name}: first node is not a BODY ({:?})",
            nodes
                .first()
                .map(|n| format!("{n:?}").chars().take(20).collect::<String>())
        )
    })?;
    let text = TextWriter::new(3701243, res_size, res_linear)
        .write(&nodes)
        .map_err(|error| format!("{name}: write: {error}"))?;
    for line in text.lines() {
        assert!(!line.ends_with(' '), "{name}: line ends in a space");
    }
    // Set XT_WRITER_OUT to keep the written files for the Parasolid cross-check
    // (`oracle/run.sh`).
    if let Some(dir) = std::env::var_os("XT_WRITER_OUT") {
        fs::create_dir_all(&dir).expect("output directory");
        fs::write(PathBuf::from(dir).join(format!("{name}.x_t")), &text).expect("write output");
    }
    let again = nodes_of(text.as_bytes());
    if again != nodes {
        return Err(format!("{name}: nodes differ after the round trip"));
    }
    Ok(())
}

#[test]
fn minimal_fixtures_round_trip() {
    let files = fixture_files("pk-minimal");
    assert_eq!(files.len(), 5);
    for (name, bytes) in files {
        round_trip(&name, &bytes).unwrap();
    }
}

#[test]
fn unusual_fixtures_round_trip_or_are_refused_with_a_reason() {
    let files = fixture_files("pk-unusual");
    let (mut written, mut refused) = (0, Vec::new());
    for (name, bytes) in &files {
        match round_trip(name, bytes) {
            Ok(()) => written += 1,
            Err(reason) => refused.push(reason),
        }
    }
    println!(
        "unusual: {written} round-tripped, {} refused",
        refused.len()
    );
    for reason in &refused {
        println!("  {reason}");
    }
    // These three have a multi-part transmit block as root instead of a BODY.
    let unsupported = [
        "xt-u-bool-edge-man",
        "xt-u-multi-lump-split",
        "xt-u-multi-lump-unite",
    ];
    assert_eq!(refused.len(), unsupported.len(), "{refused:#?}");
    for (reason, name) in refused.iter().zip(unsupported) {
        assert!(reason.starts_with(name), "{reason}");
    }
    assert_eq!(written, files.len() - unsupported.len());
}

fn fixture_nodes(name: &str) -> Vec<Node> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pk-minimal")
        .join(format!("{name}.x_t"));
    nodes_of(&fs::read(path).expect("fixture"))
}

fn write(nodes: &[Node]) -> Result<String, WriteError> {
    TextWriter::new(3701243, 1e3, 1e-8).write(nodes)
}

#[test]
fn refuses_a_non_unit_direction() {
    let mut nodes = fixture_nodes("r49-xtmin-block");
    for node in &mut nodes {
        if let Node::Plane(plane) = node {
            plane.normal = XtVector(Some([0.0, 0.0, 2.0]));
        }
    }
    assert!(matches!(
        write(&nodes),
        Err(WriteError::NonUnitDirection { .. })
    ));
}

#[test]
fn refuses_a_resolution_other_than_the_session() {
    let nodes = fixture_nodes("r49-xtmin-block");
    let error = TextWriter::new(3701243, 1e4, 1e-8)
        .write(&nodes)
        .unwrap_err();
    assert!(matches!(
        error,
        WriteError::ResolutionMismatch {
            field: "res_size",
            ..
        }
    ));
}

#[test]
fn refuses_duplicate_and_missing_indices() {
    let mut nodes = fixture_nodes("r49-xtmin-block");
    let extra = nodes.last().cloned().unwrap();
    nodes.push(extra);
    assert!(matches!(
        write(&nodes),
        Err(WriteError::DuplicateIndex { .. })
    ));

    let mut nodes = fixture_nodes("r49-xtmin-block");
    nodes.pop();
    assert!(matches!(
        write(&nodes),
        Err(WriteError::MissingIndex { .. })
    ));

    let mut nodes = fixture_nodes("r49-xtmin-block");
    nodes.swap(0, 1);
    assert_eq!(write(&nodes), Err(WriteError::FirstNodeNotBody));
}

#[test]
fn refuses_a_tolerant_edge_that_keeps_its_curve() {
    let mut nodes = fixture_nodes("r49-xtmin-block");
    for node in &mut nodes {
        if let Node::Edge(edge) = node {
            edge.tolerance = XtDouble(Some(1e-4));
        }
    }
    assert!(matches!(
        write(&nodes),
        Err(WriteError::TolerantEdgeKeepsCurve { .. })
    ));
}

#[test]
fn refuses_a_vertex_tolerance_below_its_edges() {
    let mut nodes = fixture_nodes("r49-xtmin-tolblock");
    for node in &mut nodes {
        if let Node::Vertex(vertex) = node {
            vertex.tolerance = XtDouble(Some(1e-5));
        }
    }
    assert!(matches!(
        write(&nodes),
        Err(WriteError::VertexToleranceBelowEdge { .. })
    ));
}

#[test]
fn refuses_a_tolerant_edge_whose_halfedge_lacks_a_curve() {
    let mut nodes = fixture_nodes("r49-xtmin-tolblock");
    for node in &mut nodes {
        if let Node::Halfedge(halfedge) = node {
            halfedge.curve = XtPointer::Null;
        }
    }
    assert!(matches!(
        write(&nodes),
        Err(WriteError::TolerantEdgeFinWithoutCurve { .. })
    ));
}
