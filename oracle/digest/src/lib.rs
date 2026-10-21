//! Neutral per-file description of a parsed or received Parasolid part.
//!
//! The native parser and the Parasolid oracle each produce a `FileDigest`;
//! `compare` decides whether the two agree. Neither producer sees the other.

pub mod compare;

use serde::{Deserialize, Serialize};

/// One file as seen by one producer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDigest {
    pub path: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    /// The producer described every body.
    Described { bodies: Vec<BodyDigest> },
    /// The producer ran and refused or failed; for the oracle this means silent.
    Failed { stage: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyDigest {
    pub body_type: String,
    pub counts: Counts,
    /// Axis-aligned bounds: native from vertices, Parasolid from the kernel.
    pub bounds: Option<[f64; 6]>,
    pub faces: Vec<FaceDigest>,
    pub edges: Vec<EdgeDigest>,
    pub vertices: Vec<VertexDigest>,
    /// Problems the producer noticed while describing the body.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub regions: usize,
    pub shells: usize,
    pub faces: usize,
    pub loops: usize,
    pub edges: usize,
    pub vertices: usize,
    pub fins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceDigest {
    /// Entity identifier; 0 when the producer has none.
    pub id: i32,
    /// `None` for a face with no surface.
    pub surface: Option<Geometry>,
    /// True when the face normal is the surface normal.
    pub same_sense: Option<bool>,
    pub loops: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDigest {
    pub id: i32,
    pub curve: Option<Geometry>,
    /// `None` is exact.
    pub tolerance: Option<f64>,
    pub fins: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexDigest {
    pub id: i32,
    /// `None` for a vertex with no point.
    pub position: Option<[f64; 3]>,
    pub tolerance: Option<f64>,
}

/// A carrier's class and, for analytic classes, its defining numbers in a
/// fixed order (see `compare::PARAMETER_LAYOUT`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub class: Class,
    pub params: Option<Vec<f64>>,
}

/// The shared class vocabulary both producers map into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    Line,
    Circle,
    Ellipse,
    Bspline,
    Offset,
    Swept,
    Spun,
    Blend,
    Foreign,
    Mesh,
    Intersection,
    SpCurve,
    Trimmed,
    Polyline,
    /// A class one producer has no counterpart for; always a mismatch.
    Unmapped,
}
