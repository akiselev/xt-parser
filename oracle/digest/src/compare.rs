//! Comparison of a native digest against an oracle digest.

use std::collections::BTreeMap;

use crate::{BodyDigest, Class, FileDigest, Geometry, Outcome};

/// Absolute-plus-relative tolerance for every compared real number:
/// `|a - b| <= TOL * (1 + max(|a|, |b|))`.
pub const TOL: f64 = 1e-9;

/// Parameter order per analytic class.
pub const PARAMETER_LAYOUT: &[(Class, &str)] = &[
    (Class::Plane, "origin[3] normal[3]"),
    (Class::Cylinder, "origin[3] axis[3] radius"),
    (Class::Cone, "origin[3] axis[3] radius half_angle"),
    (Class::Sphere, "centre[3] radius"),
    (Class::Torus, "centre[3] axis[3] major minor"),
    (Class::Line, "origin[3] direction[3]"),
    (Class::Circle, "centre[3] normal[3] radius"),
    (Class::Ellipse, "centre[3] normal[3] major minor"),
];

/// The categories a file can fail in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    BodyCount,
    BodyType,
    Counts,
    SurfaceClasses,
    CurveClasses,
    SurfaceParams,
    CurveParams,
    VertexPositions,
    Tolerances,
    Bounds,
    FaceSense,
    FaceLoops,
    EdgeFins,
    NativeNotes,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Self::BodyCount => "body_count",
            Self::BodyType => "body_type",
            Self::Counts => "counts",
            Self::SurfaceClasses => "surface_classes",
            Self::CurveClasses => "curve_classes",
            Self::SurfaceParams => "surface_params",
            Self::CurveParams => "curve_params",
            Self::VertexPositions => "vertex_positions",
            Self::Tolerances => "tolerances",
            Self::Bounds => "bounds",
            Self::FaceSense => "face_sense",
            Self::FaceLoops => "face_loops",
            Self::EdgeFins => "edge_fins",
            Self::NativeNotes => "native_notes",
        }
    }
}

/// Final classification of one file.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Pass,
    Fail(Vec<(Category, String)>),
    /// Parasolid did not produce a usable description; never a pass.
    OracleSilent {
        stage: String,
        message: String,
    },
    NativeError {
        stage: String,
        message: String,
    },
}

/// Relative tolerance for entity tolerances, which are tiny numbers an
/// absolute test would not distinguish: `|a - b| <= TOL * max(|a|, |b|)`.
pub fn close_relative(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL * a.abs().max(b.abs())
}

pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL * (1.0 + a.abs().max(b.abs()))
}

fn close_all(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| close(*x, *y))
}

fn multiset(classes: impl Iterator<Item = Class>) -> BTreeMap<Class, usize> {
    let mut map = BTreeMap::new();
    for class in classes {
        *map.entry(class).or_insert(0) += 1;
    }
    map
}

pub fn compare_files(native: &FileDigest, oracle: Option<&FileDigest>) -> Verdict {
    let native_bodies = match &native.outcome {
        Outcome::Described { bodies } => bodies,
        Outcome::Failed { stage, message } => {
            return Verdict::NativeError {
                stage: stage.clone(),
                message: message.clone(),
            };
        }
    };
    let oracle_bodies = match oracle.map(|o| &o.outcome) {
        Some(Outcome::Described { bodies }) => bodies,
        Some(Outcome::Failed { stage, message }) => {
            return Verdict::OracleSilent {
                stage: stage.clone(),
                message: message.clone(),
            };
        }
        None => {
            return Verdict::OracleSilent {
                stage: "missing".to_owned(),
                message: "no oracle record for this file".to_owned(),
            };
        }
    };
    let mut failures = Vec::new();
    if native_bodies.len() != oracle_bodies.len() {
        failures.push((
            Category::BodyCount,
            format!(
                "native {} oracle {}",
                native_bodies.len(),
                oracle_bodies.len()
            ),
        ));
    } else {
        for (n, o) in native_bodies.iter().zip(oracle_bodies) {
            compare_body(n, o, &mut failures);
        }
    }
    if failures.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail(failures)
    }
}

fn compare_body(n: &BodyDigest, o: &BodyDigest, out: &mut Vec<(Category, String)>) {
    if n.body_type != o.body_type {
        out.push((
            Category::BodyType,
            format!("native {} oracle {}", n.body_type, o.body_type),
        ));
    }
    for note in &n.notes {
        out.push((Category::NativeNotes, note.clone()));
    }
    if n.counts != o.counts {
        out.push((
            Category::Counts,
            format!("native {:?} oracle {:?}", n.counts, o.counts),
        ));
    }

    let n_surf = multiset(n.faces.iter().map(|f| f.surface.class));
    let o_surf = multiset(o.faces.iter().map(|f| f.surface.class));
    if n_surf != o_surf {
        out.push((
            Category::SurfaceClasses,
            format!("native {n_surf:?} oracle {o_surf:?}"),
        ));
    }
    let n_curve = multiset(
        n.edges
            .iter()
            .filter_map(|e| e.curve.as_ref())
            .map(|g| g.class),
    );
    let o_curve = multiset(
        o.edges
            .iter()
            .filter_map(|e| e.curve.as_ref())
            .map(|g| g.class),
    );
    if n_curve != o_curve {
        out.push((
            Category::CurveClasses,
            format!("native {n_curve:?} oracle {o_curve:?}"),
        ));
    }

    let by_id = ids_agree(n.faces.iter().map(|f| f.id), o.faces.iter().map(|f| f.id));
    let faces = pair(&n.faces, &o.faces, by_id, |f| f.id, |f| &f.surface);
    for (nf, of) in &faces {
        if let Some(reason) = geometry_difference(&nf.surface, &of.surface) {
            out.push((Category::SurfaceParams, format!("face {}: {reason}", nf.id)));
            break;
        }
    }
    for (nf, of) in &faces {
        if nf.surface.class == of.surface.class
            && nf.same_sense.is_some()
            && of.same_sense.is_some()
            && nf.same_sense != of.same_sense
        {
            out.push((
                Category::FaceSense,
                format!(
                    "face {} native {:?} oracle {:?}",
                    nf.id, nf.same_sense, of.same_sense
                ),
            ));
            break;
        }
    }
    for (nf, of) in &faces {
        if nf.surface.class == of.surface.class && nf.loops != of.loops {
            out.push((
                Category::FaceLoops,
                format!("face {} native {} oracle {}", nf.id, nf.loops, of.loops),
            ));
            break;
        }
    }

    let edge_by_id = ids_agree(n.edges.iter().map(|e| e.id), o.edges.iter().map(|e| e.id));
    if edge_by_id {
        let o_by_id: BTreeMap<i32, _> = o.edges.iter().map(|e| (e.id, e)).collect();
        for ne in &n.edges {
            let Some(oe) = o_by_id.get(&ne.id) else {
                continue;
            };
            if let (Some(a), Some(b)) = (&ne.curve, &oe.curve)
                && let Some(reason) = geometry_difference(a, b)
            {
                out.push((Category::CurveParams, format!("edge {}: {reason}", ne.id)));
                break;
            }
        }
        for ne in &n.edges {
            let Some(oe) = o_by_id.get(&ne.id) else {
                continue;
            };
            if ne.fins != oe.fins {
                out.push((
                    Category::EdgeFins,
                    format!("edge {} native {} oracle {}", ne.id, ne.fins, oe.fins),
                ));
                break;
            }
        }
        for ne in &n.edges {
            let Some(oe) = o_by_id.get(&ne.id) else {
                continue;
            };
            if !tolerance_agrees(ne.tolerance, oe.tolerance) {
                out.push((
                    Category::Tolerances,
                    format!(
                        "edge {} native {:?} oracle {:?}",
                        ne.id, ne.tolerance, oe.tolerance
                    ),
                ));
                break;
            }
        }
    } else {
        // Without usable identifiers compare the multisets of tolerances.
        let mut a: Vec<f64> = n.edges.iter().map(|e| e.tolerance.unwrap_or(0.0)).collect();
        let mut b: Vec<f64> = o.edges.iter().map(|e| e.tolerance.unwrap_or(0.0)).collect();
        a.sort_by(f64::total_cmp);
        b.sort_by(f64::total_cmp);
        if a.len() != b.len()
            || a.iter()
                .zip(&b)
                .any(|(x, y)| !tolerance_agrees(Some(*x), Some(*y)))
        {
            out.push((
                Category::Tolerances,
                "edge tolerance multisets differ".to_owned(),
            ));
        }
    }

    let vertex_by_id = ids_agree(
        n.vertices.iter().map(|v| v.id),
        o.vertices.iter().map(|v| v.id),
    );
    if vertex_by_id {
        let o_by_id: BTreeMap<i32, _> = o.vertices.iter().map(|v| (v.id, v)).collect();
        for nv in &n.vertices {
            let Some(ov) = o_by_id.get(&nv.id) else {
                continue;
            };
            if !close_all(&nv.position, &ov.position) {
                out.push((
                    Category::VertexPositions,
                    format!(
                        "vertex {} native {:?} oracle {:?}",
                        nv.id, nv.position, ov.position
                    ),
                ));
                break;
            }
        }
        for nv in &n.vertices {
            let Some(ov) = o_by_id.get(&nv.id) else {
                continue;
            };
            if !tolerance_agrees(nv.tolerance, ov.tolerance) {
                out.push((
                    Category::Tolerances,
                    format!(
                        "vertex {} native {:?} oracle {:?}",
                        nv.id, nv.tolerance, ov.tolerance
                    ),
                ));
                break;
            }
        }
    } else {
        let mut a: Vec<[f64; 3]> = n.vertices.iter().map(|v| v.position).collect();
        let mut b: Vec<[f64; 3]> = o.vertices.iter().map(|v| v.position).collect();
        let key = |p: &[f64; 3], q: &[f64; 3]| {
            p[0].total_cmp(&q[0])
                .then(p[1].total_cmp(&q[1]))
                .then(p[2].total_cmp(&q[2]))
        };
        a.sort_by(key);
        b.sort_by(key);
        if a.len() != b.len() || a.iter().zip(&b).any(|(p, q)| !close_all(p, q)) {
            out.push((
                Category::VertexPositions,
                "vertex multisets differ".to_owned(),
            ));
        }
    }

    if let (Some(nb), Some(ob)) = (n.bounds, o.bounds) {
        // The kernel box may exceed the vertex box on curved bodies, so the
        // check is containment of the native box within the oracle box.
        let slack = |a: f64, b: f64| TOL * (1.0 + a.abs().max(b.abs()));
        let contained = (0..3).all(|i| {
            ob[i] <= nb[i] + slack(ob[i], nb[i])
                && nb[i + 3] <= ob[i + 3] + slack(ob[i + 3], nb[i + 3])
        });
        if !contained {
            out.push((
                Category::Bounds,
                format!("native {nb:?} not within oracle {ob:?}"),
            ));
        }
    }
}

fn tolerance_agrees(native: Option<f64>, oracle: Option<f64>) -> bool {
    match (native, oracle) {
        (None, None) => true,
        (Some(a), Some(b)) => close_relative(a, b),
        _ => false,
    }
}

/// Identifiers can pair entities only when both sides have the same set of
/// nonzero, unique values.
fn ids_agree(native: impl Iterator<Item = i32>, oracle: impl Iterator<Item = i32>) -> bool {
    let mut a: Vec<i32> = native.collect();
    let mut b: Vec<i32> = oracle.collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b && a.iter().all(|id| *id != 0) && a.windows(2).all(|w| w[0] != w[1])
}

/// Pair entities by identifier when `by_id`, otherwise by (class, params) order.
fn pair<'a, T>(
    native: &'a [T],
    oracle: &'a [T],
    by_id: bool,
    id: impl Fn(&T) -> i32,
    geometry: impl Fn(&T) -> &Geometry,
) -> Vec<(&'a T, &'a T)> {
    if by_id {
        let o_by_id: BTreeMap<i32, &T> = oracle.iter().map(|t| (id(t), t)).collect();
        return native
            .iter()
            .filter_map(|n| o_by_id.get(&id(n)).map(|o| (n, *o)))
            .collect();
    }
    let order = |a: &&T, b: &&T| {
        let (ga, gb) = (geometry(a), geometry(b));
        ga.class.cmp(&gb.class).then_with(|| {
            let pa = ga.params.as_deref().unwrap_or(&[]);
            let pb = gb.params.as_deref().unwrap_or(&[]);
            for (x, y) in pa.iter().zip(pb) {
                let ord = x.total_cmp(y);
                if ord.is_ne() {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        })
    };
    let mut a: Vec<&T> = native.iter().collect();
    let mut b: Vec<&T> = oracle.iter().collect();
    a.sort_by(order);
    b.sort_by(order);
    a.into_iter().zip(b).collect()
}

fn geometry_difference(native: &Geometry, oracle: &Geometry) -> Option<String> {
    if native.class != oracle.class {
        return Some(format!(
            "class native {:?} oracle {:?}",
            native.class, oracle.class
        ));
    }
    match (&native.params, &oracle.params) {
        (Some(a), Some(b)) if !close_all(a, b) => {
            Some(format!("{:?} native {a:?} oracle {b:?}", native.class))
        }
        (Some(_), None) | (None, Some(_)) => {
            Some(format!("{:?} params present on one side", native.class))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Counts, EdgeDigest, FaceDigest, VertexDigest};

    fn plane(origin: [f64; 3], normal: [f64; 3]) -> Geometry {
        let mut params = origin.to_vec();
        params.extend(normal);
        Geometry {
            class: Class::Plane,
            params: Some(params),
        }
    }

    fn body(id: i32, surface: Geometry) -> BodyDigest {
        BodyDigest {
            body_type: "solid".to_owned(),
            counts: Counts {
                regions: 2,
                shells: 2,
                faces: 1,
                loops: 1,
                edges: 1,
                vertices: 1,
                fins: 2,
            },
            bounds: Some([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]),
            faces: vec![FaceDigest {
                id,
                surface,
                same_sense: Some(true),
                loops: 1,
            }],
            edges: vec![EdgeDigest {
                id: 5,
                curve: None,
                tolerance: None,
                fins: 2,
            }],
            vertices: vec![VertexDigest {
                id: 9,
                position: [0.0, 0.0, 0.0],
                tolerance: None,
            }],
            notes: Vec::new(),
        }
    }

    fn file(body: BodyDigest) -> FileDigest {
        FileDigest {
            path: "x".to_owned(),
            outcome: Outcome::Described { bodies: vec![body] },
        }
    }

    #[test]
    fn identical_digests_pass() {
        let a = file(body(3, plane([0.0; 3], [0.0, 0.0, 1.0])));
        assert_eq!(compare_files(&a, Some(&a.clone())), Verdict::Pass);
    }

    #[test]
    fn plane_normal_difference_is_a_surface_param_failure() {
        let a = file(body(3, plane([0.0; 3], [0.0, 0.0, 1.0])));
        let b = file(body(3, plane([0.0; 3], [0.0, 1.0, 0.0])));
        match compare_files(&a, Some(&b)) {
            Verdict::Fail(list) => assert_eq!(list[0].0, Category::SurfaceParams),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn differences_inside_tolerance_pass() {
        let a = file(body(3, plane([0.0; 3], [0.0, 0.0, 1.0])));
        let b = file(body(3, plane([1e-12, 0.0, 0.0], [0.0, 0.0, 1.0])));
        assert_eq!(compare_files(&a, Some(&b)), Verdict::Pass);
    }

    #[test]
    fn missing_or_failed_oracle_is_silent_never_pass() {
        let a = file(body(3, plane([0.0; 3], [0.0, 0.0, 1.0])));
        assert!(matches!(
            compare_files(&a, None),
            Verdict::OracleSilent { .. }
        ));
        let failed = FileDigest {
            path: "x".to_owned(),
            outcome: Outcome::Failed {
                stage: "receive".to_owned(),
                message: "bad".to_owned(),
            },
        };
        assert!(matches!(
            compare_files(&a, Some(&failed)),
            Verdict::OracleSilent { .. }
        ));
    }

    #[test]
    fn unset_and_set_tolerance_disagree() {
        assert!(tolerance_agrees(None, None));
        assert!(!tolerance_agrees(None, Some(1e-6)));
        assert!(tolerance_agrees(Some(1e-6), Some(1e-6)));
    }

    #[test]
    fn fewer_bodies_is_a_body_count_failure() {
        let a = file(body(3, plane([0.0; 3], [0.0, 0.0, 1.0])));
        let b = FileDigest {
            path: "x".to_owned(),
            outcome: Outcome::Described { bodies: vec![] },
        };
        match compare_files(&a, Some(&b)) {
            Verdict::Fail(list) => assert_eq!(list[0].0, Category::BodyCount),
            other => panic!("{other:?}"),
        }
    }
}
