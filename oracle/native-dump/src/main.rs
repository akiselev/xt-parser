//! Describe XT files with the native parser and write one JSON digest per line.
//!
//! Usage: xt-native-dump LIST OUT [--threads N]
//! LIST holds one `.x_t` path per line.

use std::collections::BTreeSet;
use std::io::Write;
use std::sync::Mutex;

use xt_schema::view::{
    CurveKind, SurfaceKind, ViewError,
    sch_30100::{Curve, LoopBoundary, Model, Surface},
};
use xt_xcheck_digest::{
    BodyDigest, Class, Counts, EdgeDigest, FaceDigest, FileDigest, Geometry, Outcome, VertexDigest,
};

fn main() {
    if let Err(message) = run() {
        eprintln!("xt-native-dump: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let list = args
        .next()
        .ok_or("usage: xt-native-dump LIST OUT [--threads N]")?;
    let out = args.next().ok_or("missing OUT")?;
    let mut threads = 4usize;
    while let Some(flag) = args.next() {
        if flag == "--threads" {
            threads = args
                .next()
                .and_then(|v| v.parse().ok())
                .filter(|n| *n > 0)
                .ok_or("--threads needs a positive integer")?;
        } else {
            return Err(format!("unknown argument {flag}"));
        }
    }
    let paths: Vec<String> = std::fs::read_to_string(&list)
        .map_err(|e| format!("read {list}: {e}"))?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let results: Mutex<Vec<Option<FileDigest>>> = Mutex::new(vec![None; paths.len()]);
    std::thread::scope(|scope| {
        for worker in 0..threads {
            let (paths, results) = (&paths, &results);
            scope.spawn(move || {
                for index in (worker..paths.len()).step_by(threads) {
                    let digest = describe_file(&paths[index]);
                    if let Ok(mut guard) = results.lock() {
                        guard[index] = Some(digest);
                    }
                }
            });
        }
    });
    let results = results.into_inner().map_err(|e| e.to_string())?;
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(&out).map_err(|e| format!("create {out}: {e}"))?,
    );
    for digest in results.into_iter().flatten() {
        let line = serde_json::to_string(&digest).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())
}

fn describe_file(path: &str) -> FileDigest {
    let failed = |stage: &str, message: String| FileDigest {
        path: path.to_owned(),
        outcome: Outcome::Failed {
            stage: stage.to_owned(),
            message,
        },
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return failed("read", error.to_string()),
    };
    let document = match xt_schema::parse_text_30100(&bytes) {
        Ok(document) => document,
        Err(error) => return failed("parse", error.to_string()),
    };
    let model = match Model::from_text(&document) {
        Ok(model) => model,
        Err(error) => return failed("model", error.to_string()),
    };
    let mut bodies = Vec::new();
    for body in model.bodies() {
        match describe_body(body) {
            Ok(digest) => bodies.push(digest),
            Err(error) => return failed("view", error.to_string()),
        }
    }
    FileDigest {
        path: path.to_owned(),
        outcome: Outcome::Described { bodies },
    }
}

fn id_of(value: xt_parser::XtInteger) -> i32 {
    value.0.unwrap_or(0)
}

fn describe_body(body: xt_schema::view::sch_30100::Body<'_>) -> Result<BodyDigest, ViewError> {
    let mut notes = Vec::new();
    let regions = body.regions()?;
    let mut shells = 0;
    let mut faces = Vec::new();
    let mut front_faces = BTreeSet::new();
    for region in &regions {
        let region_shells = region.shells()?;
        shells += region_shells.len();
        for shell in region_shells {
            faces.extend(shell.faces()?);
            for face in shell.front_faces()? {
                front_faces.insert(face.file_index());
            }
        }
    }
    let back: BTreeSet<u32> = faces.iter().map(|f| f.file_index()).collect();
    if back.len() != faces.len() {
        notes.push("a face is listed under more than one back shell".to_owned());
    }
    if !front_faces.is_subset(&back) {
        notes.push("a front-shell face is not on any back-shell list".to_owned());
    }

    let mut loops = 0;
    let mut fins = 0;
    let mut fin_edges = BTreeSet::new();
    let mut fin_vertices = BTreeSet::new();
    let mut face_digests = Vec::new();
    for face in &faces {
        let face_loops = face.loops()?;
        loops += face_loops.len();
        for loop_ in &face_loops {
            match loop_.boundary()? {
                LoopBoundary::Circuit(circuit) => {
                    for fin in circuit {
                        fins += 1;
                        fin_edges.insert(fin.edge()?.file_index());
                        if let Some(vertex) = fin.vertex()? {
                            fin_vertices.insert(vertex.file_index());
                        }
                    }
                }
                LoopBoundary::IsolatedVertex(vertex) => {
                    fin_vertices.insert(vertex.file_index());
                }
            }
        }
        face_digests.push(FaceDigest {
            id: id_of(face.node().node_id),
            surface: surface_geometry(face.surface()?),
            same_sense: Some(face.agrees_with_surface()?),
            loops: face_loops.len(),
        });
    }

    let edges = body.edges()?;
    let edge_set: BTreeSet<u32> = edges.iter().map(|e| e.file_index()).collect();
    if !fin_edges.is_subset(&edge_set) {
        notes.push("a loop fin uses an edge that is not on the body edge list".to_owned());
    }
    let mut edge_digests = Vec::new();
    for edge in &edges {
        edge_digests.push(EdgeDigest {
            id: id_of(edge.node().node_id),
            curve: edge.curve()?.map(curve_geometry),
            tolerance: edge.tolerance(),
            fins: edge.fins()?.len(),
        });
    }

    let vertices = body.vertices()?;
    let vertex_set: BTreeSet<u32> = vertices.iter().map(|v| v.file_index()).collect();
    if !fin_vertices.is_subset(&vertex_set) {
        notes.push("a loop fin starts at a vertex that is not on the body vertex list".to_owned());
    }
    let mut vertex_digests = Vec::new();
    for vertex in &vertices {
        vertex_digests.push(VertexDigest {
            id: id_of(vertex.node().node_id),
            position: vertex.position()?,
            tolerance: vertex.tolerance(),
        });
    }

    let bounds = vertex_digests
        .iter()
        .fold(None, |acc: Option<[f64; 6]>, v| {
            let p = v.position;
            Some(match acc {
                None => [p[0], p[1], p[2], p[0], p[1], p[2]],
                Some(b) => [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].min(p[2]),
                    b[3].max(p[0]),
                    b[4].max(p[1]),
                    b[5].max(p[2]),
                ],
            })
        });

    Ok(BodyDigest {
        body_type: format!("{:?}", body.kind()?).to_lowercase(),
        counts: Counts {
            regions: regions.len(),
            shells,
            faces: faces.len(),
            loops,
            edges: edges.len(),
            vertices: vertices.len(),
            fins,
        },
        bounds,
        faces: face_digests,
        edges: edge_digests,
        vertices: vertex_digests,
        notes,
    })
}

fn flat(parts: &[&[f64]]) -> Option<Vec<f64>> {
    Some(parts.iter().flat_map(|p| p.iter().copied()).collect())
}

fn surface_geometry(surface: Surface<'_>) -> Geometry {
    let (class, params) = match surface {
        Surface::Plane(s) => (Class::Plane, flat(&[&s.origin, &s.normal])),
        Surface::Cylinder(s) => (Class::Cylinder, flat(&[&s.origin, &s.axis, &[s.radius]])),
        Surface::Cone(s) => (
            Class::Cone,
            flat(&[&s.origin, &s.axis, &[s.radius, s.half_angle]]),
        ),
        Surface::Sphere(s) => (Class::Sphere, flat(&[&s.centre, &[s.radius]])),
        Surface::Torus(s) => (
            Class::Torus,
            flat(&[&s.centre, &s.axis, &[s.major_radius, s.minor_radius]]),
        ),
        Surface::Other(other) => (
            match other.kind {
                SurfaceKind::Offset => Class::Offset,
                SurfaceKind::Swept => Class::Swept,
                SurfaceKind::Spun => Class::Spun,
                SurfaceKind::Pipe
                | SurfaceKind::BlendedEdge
                | SurfaceKind::BlendedVertex
                | SurfaceKind::BlendOverlap
                | SurfaceKind::BlendBound => Class::Blend,
                SurfaceKind::Foreign => Class::Foreign,
                SurfaceKind::Bspline => Class::Bspline,
                SurfaceKind::Mesh => Class::Mesh,
                _ => Class::Unmapped,
            },
            None,
        ),
    };
    Geometry { class, params }
}

fn curve_geometry(curve: Curve<'_>) -> Geometry {
    let (class, params) = match curve {
        Curve::Line(c) => (Class::Line, flat(&[&c.origin, &c.direction])),
        Curve::Circle(c) => (Class::Circle, flat(&[&c.centre, &c.normal, &[c.radius]])),
        Curve::Ellipse(c) => (
            Class::Ellipse,
            flat(&[&c.centre, &c.normal, &[c.major_radius, c.minor_radius]]),
        ),
        Curve::Other(other) => (
            match other.kind {
                CurveKind::Bspline => Class::Bspline,
                CurveKind::Intersection => Class::Intersection,
                CurveKind::SpCurve => Class::SpCurve,
                CurveKind::Trimmed => Class::Trimmed,
                CurveKind::Polyline => Class::Polyline,
                CurveKind::Foreign => Class::Foreign,
                CurveKind::Offset => Class::Offset,
                _ => Class::Unmapped,
            },
            None,
        ),
    };
    Geometry { class, params }
}
