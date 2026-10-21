// Included once per generated schema; `view` and `Node` come from the including module.

use std::collections::BTreeSet;

use view::{Curve, LoopBoundary, Model, Surface};
use xt_schema::TypedTextDocument;
use xt_schema::view::{CurveKind, SurfaceKind, ViewError};
use xt_xcheck_digest::{
    BodyDigest, Class, Counts, EdgeDigest, FaceDigest, Geometry, VertexDigest,
};

/// Describe every body of a parsed document.
pub fn describe(document: &TypedTextDocument<Node>) -> Result<Vec<BodyDigest>, (&'static str, String)> {
    let model = Model::from_text(document).map_err(|e| ("model", e.to_string()))?;
    model
        .bodies()
        .map(|body| describe_body(body).map_err(|e| ("view", e.to_string())))
        .collect()
}

fn id_of(value: xt_parser::XtInteger) -> i32 {
    value.0.unwrap_or(0)
}

fn describe_body(body: view::Body<'_>) -> Result<BodyDigest, ViewError> {
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
                        if let Some(vertex) = fin.end_vertex()? {
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
            surface: face.surface()?.map(surface_geometry),
            same_sense: face.agrees_with_surface()?,
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
        .filter_map(|v| v.position)
        .fold(None, |acc: Option<[f64; 6]>, p| {
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
