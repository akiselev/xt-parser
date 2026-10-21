//! Receive XT files into Parasolid and write one JSON digest per line.
//!
//! Build for `x86_64-pc-windows-gnu` and run under Wine:
//!   xt-pk-dump LIST OUT
//! LIST holds one path per line (Windows or `Z:` style). Lines of OUT are
//! appended and flushed after every file, so a crashed run can be resumed by
//! passing only the files not yet in OUT.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use parasolid::{
    Behaviour, Body, CurveType, FrustrumConfig, Session, SessionConfig, SurfType, fileio,
};
use xt_xcheck_digest::{
    BodyDigest, Class, Counts, EdgeDigest, FaceDigest, FileDigest, Geometry, Outcome, VertexDigest,
};

type Described<T> = Result<T, String>;

fn main() {
    if let Err(message) = run() {
        eprintln!("xt-pk-dump: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let list = args.next().ok_or("usage: xt-pk-dump LIST OUT")?;
    let out = args.next().ok_or("missing OUT")?;
    let paths: Vec<String> = fs::read_to_string(&list)
        .map_err(|e| format!("read {list}: {e}"))?
        .lines()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    let staging = staging_dir()?;
    let session = Session::start(
        SessionConfig::new()
            .behaviour(Behaviour::Latest)
            .frustrum(FrustrumConfig::new().base_dir(&staging))
            .check_arguments(true)
            .general_topology(true),
    )
    .map_err(|e| format!("session: {e}"))?;
    let mut sink = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out)
        .map_err(|e| format!("open {out}: {e}"))?;
    // Parasolid reports an exact (non-tolerant) edge or vertex with half the
    // session linear precision; anything larger is a real tolerance.
    let exact_precision = session
        .precision()
        .map_err(|e| format!("session precision: {e}"))?
        / 2.0;
    for (ordinal, path) in paths.iter().enumerate() {
        let digest = describe_file(path, &staging, ordinal, exact_precision);
        let line = serde_json::to_string(&digest).map_err(|e| e.to_string())?;
        writeln!(sink, "{line}").map_err(|e| e.to_string())?;
        sink.flush().map_err(|e| e.to_string())?;
    }
    drop(session);
    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

fn staging_dir() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("xt-pk-dump-{}-{nonce}", std::process::id()));
    fs::create_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn describe_file(path: &str, staging: &Path, ordinal: usize, exact: f64) -> FileDigest {
    let failed = |stage: &str, message: String| FileDigest {
        path: path.to_owned(),
        outcome: Outcome::Failed {
            stage: stage.to_owned(),
            message,
        },
    };
    let key = format!("part{ordinal}");
    let staged = staging.join(format!("{key}.xmt_txt"));
    if let Err(error) = fs::copy(path, &staged) {
        return failed("stage", error.to_string());
    }
    let received = fileio::receive(&key);
    let _ = fs::remove_file(&staged);
    let bodies = match received {
        Ok(bodies) => bodies,
        Err(error) => return failed("receive", error.to_string()),
    };
    let mut described = Vec::new();
    let mut failure = None;
    for body in &bodies {
        match describe_body(body, exact) {
            Ok(digest) => described.push(digest),
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    for body in bodies {
        let _ = body.delete();
    }
    match failure {
        Some(error) => failed("query", error),
        None => FileDigest {
            path: path.to_owned(),
            outcome: Outcome::Described { bodies: described },
        },
    }
}

fn q<T>(what: &str, result: parasolid::PsResult<T>) -> Described<T> {
    result.map_err(|e| format!("{what}: {e}"))
}

fn describe_body(body: &Body, exact: f64) -> Described<BodyDigest> {
    let body_type = format!("{:?}", q("body type", body.body_type())?).to_lowercase();
    let regions = q("regions", body.regions())?;
    let mut shells = 0;
    for region in &regions {
        shells += q("shells", region.shells())?.len();
    }
    let faces = q("faces", body.faces())?;
    let mut loops = 0;
    let mut fins = 0;
    let mut face_digests = Vec::new();
    for face in &faces {
        let face_loops = q("loops", face.loops())?;
        loops += face_loops.len();
        for loop_ in &face_loops {
            fins += q("fins", loop_.fins())?.len();
        }
        let surface = if q("surf", face.surf_tag())? == 0 {
            None
        } else {
            Some(surface_geometry(&q("surf", face.surf())?)?)
        };
        face_digests.push(FaceDigest {
            id: q("face id", face.entity().identifier())?,
            surface,
            same_sense: Some(q("orientation", face.orientation())?),
            loops: face_loops.len(),
        });
    }
    let edges = q("edges", body.edges())?;
    let mut edge_digests = Vec::new();
    for edge in &edges {
        let curve_tag = q("edge curve", edge.curve_tag())?;
        let curve = if curve_tag == 0 {
            None
        } else {
            Some(curve_geometry(&q("edge curve", edge.curve())?)?)
        };
        let precision = q("edge precision", edge.precision())?;
        edge_digests.push(EdgeDigest {
            id: q("edge id", edge.entity().identifier())?,
            curve,
            tolerance: (precision > exact * (1.0 + 1e-9)).then_some(precision),
            fins: q("edge fins", edge.fins())?.len(),
        });
    }
    let vertices = q("vertices", body.vertices())?;
    let mut vertex_digests = Vec::new();
    for vertex in &vertices {
        let mut point_tag: parasolid_sys::PK_POINT_t = 0;
        // SAFETY: the vertex tag is live and `point_tag` is a valid out slot.
        let rc = unsafe { parasolid_sys::PK_VERTEX_ask_point(vertex.tag(), &mut point_tag) };
        if rc != 0 {
            return Err(format!("vertex point: PK error {rc}"));
        }
        let position = if point_tag == 0 {
            None
        } else {
            let p = q("vertex point", vertex.point())?;
            Some([p.x, p.y, p.z])
        };
        let precision = q("vertex precision", vertex.precision())?;
        vertex_digests.push(VertexDigest {
            id: q("vertex id", vertex.entity().identifier())?,
            position,
            tolerance: (precision > exact * (1.0 + 1e-9)).then_some(precision),
        });
    }
    // Parasolid cannot box an empty body or a wire edge without a curve; that is
    // recorded rather than treated as a failed query.
    let bounds = body.bounding_box();
    let mut notes = vec![format!("exact_precision={exact:e}")];
    Ok(BodyDigest {
        body_type,
        counts: Counts {
            regions: regions.len(),
            shells,
            faces: faces.len(),
            loops,
            edges: edges.len(),
            vertices: vertices.len(),
            fins,
        },
        bounds: match &bounds {
            Ok(b) => Some([b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]),
            Err(error) => {
                notes.push(format!("bounds unavailable: {error}"));
                None
            }
        },
        faces: face_digests,
        edges: edge_digests,
        vertices: vertex_digests,
        notes,
    })
}

fn v(p: parasolid::Vec3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn flat(parts: &[&[f64]]) -> Option<Vec<f64>> {
    Some(parts.iter().flat_map(|p| p.iter().copied()).collect())
}

fn surface_geometry(surf: &parasolid::Surf) -> Described<Geometry> {
    let (class, params) = match q("surface type", surf.surf_type())? {
        SurfType::Plane => {
            let d = q("plane", surf.ask_plane())?;
            (Class::Plane, flat(&[&v(d.basis.origin), &v(d.basis.axis)]))
        }
        SurfType::Cylinder => {
            let d = q("cylinder", surf.ask_cylinder())?;
            (
                Class::Cylinder,
                flat(&[&v(d.basis.origin), &v(d.basis.axis), &[d.radius]]),
            )
        }
        SurfType::Cone => {
            let d = q("cone", surf.ask_cone())?;
            (
                Class::Cone,
                flat(&[
                    &v(d.basis.origin),
                    &v(d.basis.axis),
                    &[d.radius, d.semi_angle],
                ]),
            )
        }
        SurfType::Sphere => {
            let d = q("sphere", surf.ask_sphere())?;
            (Class::Sphere, flat(&[&v(d.basis.origin), &[d.radius]]))
        }
        SurfType::Torus => {
            let d = q("torus", surf.ask_torus())?;
            (
                Class::Torus,
                flat(&[
                    &v(d.basis.origin),
                    &v(d.basis.axis),
                    &[d.major_radius, d.minor_radius],
                ]),
            )
        }
        SurfType::Bsurf => (Class::Bspline, None),
        SurfType::Offset => (Class::Offset, None),
        SurfType::Swept => (Class::Swept, None),
        SurfType::Spun => (Class::Spun, None),
        SurfType::Fsurf => (Class::Foreign, None),
        SurfType::Mesh => (Class::Mesh, None),
        SurfType::Blendsf => (Class::Blend, None),
        SurfType::Ssurf => (Class::Unmapped, None),
    };
    Ok(Geometry { class, params })
}

fn curve_geometry(curve: &parasolid::Curve) -> Described<Geometry> {
    let (class, params) = match q("curve type", curve.curve_type())? {
        CurveType::Line => {
            let d = q("line", curve.ask_line())?;
            (Class::Line, flat(&[&v(d.origin), &v(d.direction)]))
        }
        CurveType::Circle => {
            let d = q("circle", curve.ask_circle())?;
            (
                Class::Circle,
                flat(&[&v(d.basis.origin), &v(d.basis.axis), &[d.radius]]),
            )
        }
        CurveType::Ellipse => {
            let d = q("ellipse", curve.ask_ellipse())?;
            (
                Class::Ellipse,
                flat(&[&v(d.basis.origin), &v(d.basis.axis), &[d.r1, d.r2]]),
            )
        }
        CurveType::Bcurve => (Class::Bspline, None),
        CurveType::Icurve => (Class::Intersection, None),
        CurveType::Fcurve => (Class::Foreign, None),
        CurveType::Scurve => (Class::SpCurve, None),
        CurveType::Tcurve => (Class::Trimmed, None),
        CurveType::Cpcurve => (Class::Unmapped, None),
        CurveType::Pline => (Class::Polyline, None),
    };
    Ok(Geometry { class, params })
}
