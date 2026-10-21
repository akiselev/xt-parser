//! Schema-independent vocabulary of the resolved B-rep view.

use std::fmt;

/// A point or direction in model space.
pub type Vec3 = [f64; 3];

/// Orientation of a face against its surface, or of an edge/fin against its curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sense {
    Forward,
    Reversed,
}

impl Sense {
    pub(crate) fn from_char(field: &'static str, raw: u8) -> Result<Self, ViewError> {
        match raw {
            b'+' => Ok(Self::Forward),
            b'-' => Ok(Self::Reversed),
            other => Err(ViewError::BadEnumeration {
                field,
                value: other,
            }),
        }
    }
}

/// Whether a region is occupied by material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    Solid,
    Void,
}

impl RegionKind {
    pub(crate) fn from_char(raw: u8) -> Result<Self, ViewError> {
        match raw {
            b'S' => Ok(Self::Solid),
            b'V' => Ok(Self::Void),
            other => Err(ViewError::BadEnumeration {
                field: "region.type",
                value: other,
            }),
        }
    }
}

/// The kind of a body. Only the codes seen in files and cross-checked against
/// Parasolid are named (`dynamic-observed`, 152,096 ABC parts); any other code
/// is an error rather than a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyKind {
    Solid,
    Wire,
    Sheet,
}

impl BodyKind {
    pub(crate) fn from_code(raw: u8) -> Result<Self, ViewError> {
        match raw {
            1 => Ok(Self::Solid),
            2 => Ok(Self::Wire),
            3 => Ok(Self::Sheet),
            other => Err(ViewError::BadEnumeration {
                field: "body.body_type",
                value: other,
            }),
        }
    }
}

/// Every surface class that can be the carrier of a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    Pipe,
    BlendedEdge,
    BlendedVertex,
    BlendOverlap,
    BlendBound,
    Offset,
    Parasurf,
    ObsoleteCps,
    Silhouette,
    Swept,
    Spun,
    Cps,
    ObsoleteMesh,
    Foreign,
    Bspline,
    Mesh,
}

/// Every curve class that can be the carrier of an edge or fin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CurveKind {
    Line,
    Circle,
    Ellipse,
    Parabola,
    Hyperbola,
    Paracurve,
    ObsoleteCpc,
    PatchBound,
    Intersection,
    Silhouette,
    Offset,
    Cpc,
    ObsoleteSpCurve,
    ObsoletePolyline,
    Foreign,
    Pcurve,
    Trimmed,
    Bspline,
    SpCurve,
    Polyline,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub origin: Vec3,
    pub normal: Vec3,
    pub x_axis: Option<Vec3>,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylinder {
    pub origin: Vec3,
    pub axis: Vec3,
    pub radius: f64,
    pub x_axis: Option<Vec3>,
    pub sense: Sense,
}

/// `radius` is measured at `origin`; `half_angle` is in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cone {
    pub origin: Vec3,
    pub axis: Vec3,
    pub radius: f64,
    pub half_angle: f64,
    pub x_axis: Option<Vec3>,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub centre: Vec3,
    pub radius: f64,
    pub axis: Option<Vec3>,
    pub x_axis: Option<Vec3>,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Torus {
    pub centre: Vec3,
    pub axis: Vec3,
    pub major_radius: f64,
    pub minor_radius: f64,
    pub x_axis: Option<Vec3>,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub origin: Vec3,
    pub direction: Vec3,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub centre: Vec3,
    pub normal: Vec3,
    pub x_axis: Option<Vec3>,
    pub radius: f64,
    pub sense: Sense,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipse {
    pub centre: Vec3,
    pub normal: Vec3,
    pub x_axis: Option<Vec3>,
    pub major_radius: f64,
    pub minor_radius: f64,
    pub sense: Sense,
}

/// Why a view query failed. The input document is not at fault only when the
/// error is `Unsupported`; the others describe a malformed graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    RecordCountMismatch {
        records: usize,
        nodes: usize,
    },
    DuplicateIndex {
        index: u32,
    },
    DanglingPointer {
        field: &'static str,
        index: u32,
    },
    UnsetPointer {
        field: &'static str,
    },
    NullPointer {
        field: &'static str,
    },
    WrongNode {
        field: &'static str,
        expected: &'static str,
        found: String,
    },
    UnsetValue {
        field: &'static str,
    },
    BadEnumeration {
        field: &'static str,
        value: u8,
    },
    Cycle {
        field: &'static str,
    },
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordCountMismatch { records, nodes } => {
                write!(f, "{records} raw records but {nodes} typed nodes")
            }
            Self::DuplicateIndex { index } => write!(f, "duplicate node index {index}"),
            Self::DanglingPointer { field, index } => {
                write!(f, "{field}: no node with index {index}")
            }
            Self::UnsetPointer { field } => write!(f, "{field}: unset pointer"),
            Self::NullPointer { field } => write!(f, "{field}: required pointer is null"),
            Self::WrongNode {
                field,
                expected,
                found,
            } => write!(f, "{field}: expected {expected}, found {found}"),
            Self::UnsetValue { field } => write!(f, "{field}: required value is unset"),
            Self::BadEnumeration { field, value } => {
                write!(f, "{field}: unexpected character {:?}", *value as char)
            }
            Self::Cycle { field } => write!(f, "{field}: chain does not terminate"),
        }
    }
}

impl std::error::Error for ViewError {}
