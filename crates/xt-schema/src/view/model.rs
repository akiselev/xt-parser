// Included once per generated schema module; `schema` is that module.

use std::collections::HashMap;

use schema::{Node, nodes as n};
use xt_parser::{NodeRecord, XtDouble, XtPointer, XtVector};

use super::common::{
    BodyKind, Circle, Cone, Cylinder, CurveKind, Ellipse, Line, Plane, RegionKind, Sense, Sphere,
    SurfaceKind, Torus, Vec3, ViewError,
};

type Result<T> = std::result::Result<T, ViewError>;

/// A parsed document indexed by file index so pointers can be followed.
#[derive(Debug)]
pub struct Model<'a> {
    nodes: &'a [Node],
    by_index: HashMap<u32, usize>,
}

fn kind_name(node: &Node) -> String {
    let text = format!("{node:?}");
    text.split('(').next().unwrap_or("?").to_owned()
}

fn required_vec(field: &'static str, value: XtVector) -> Result<Vec3> {
    value.0.ok_or(ViewError::UnsetValue { field })
}

fn required_f64(field: &'static str, value: XtDouble) -> Result<f64> {
    value.0.ok_or(ViewError::UnsetValue { field })
}

impl<'a> Model<'a> {
    /// `records` and `nodes` are the raw and typed forms of one document, in order.
    pub fn new(records: &[NodeRecord], nodes: &'a [Node]) -> Result<Self> {
        if records.len() != nodes.len() {
            return Err(ViewError::RecordCountMismatch {
                records: records.len(),
                nodes: nodes.len(),
            });
        }
        let mut by_index = HashMap::with_capacity(nodes.len());
        for (position, record) in records.iter().enumerate() {
            if by_index.insert(record.file_index, position).is_some() {
                return Err(ViewError::DuplicateIndex {
                    index: record.file_index,
                });
            }
        }
        Ok(Self { nodes, by_index })
    }

    pub fn from_text(document: &'a crate::TypedTextDocument<Node>) -> Result<Self> {
        Self::new(&document.raw.nodes, &document.nodes)
    }

    pub fn from_binary(document: &'a crate::TypedBinaryDocument<Node>) -> Result<Self> {
        Self::new(&document.raw.nodes, &document.nodes)
    }

    /// Every body node in the document, in file order.
    pub fn bodies(&'a self) -> impl Iterator<Item = Body<'a>> + 'a {
        self.nodes.iter().filter_map(move |node| match node {
            Node::Body(body) => Some(Body { model: self, node: body }),
            _ => None,
        })
    }

    fn lookup<T>(&self, field: &'static str, pointer: XtPointer<T>) -> Result<Option<&'a Node>> {
        match pointer {
            XtPointer::Null => Ok(None),
            XtPointer::Unset => Err(ViewError::UnsetPointer { field }),
            XtPointer::Index(index) => {
                let raw = index.get();
                let position = self
                    .by_index
                    .get(&raw)
                    .ok_or(ViewError::DanglingPointer { field, index: raw })?;
                Ok(Some(&self.nodes[*position]))
            }
        }
    }

    /// The curve a pointer designates, looking through trimmed-curve wrappers.
    fn carrier<T>(&'a self, field: &'static str, pointer: XtPointer<T>) -> Result<Option<Curve<'a>>> {
        let Some(mut node) = self.lookup(field, pointer)? else {
            return Ok(None);
        };
        for _ in 0..=self.nodes.len() {
            let Node::TrimmedCurve(trimmed) = node else {
                return classify_curve(field, node).map(Some);
            };
            node = Self::require(field, self.lookup(field, trimmed.basis_curve)?)?;
        }
        Err(ViewError::Cycle { field })
    }

    fn require<V>(field: &'static str, value: Option<V>) -> Result<V> {
        value.ok_or(ViewError::NullPointer { field })
    }

    /// Follow `next` from `first` until null, failing on a cycle.
    fn chain<V: Copy>(
        &self,
        field: &'static str,
        first: Option<V>,
        next: impl Fn(V) -> Result<Option<V>>,
    ) -> Result<Vec<V>> {
        let mut out = Vec::new();
        let mut current = first;
        while let Some(item) = current {
            if out.len() > self.nodes.len() {
                return Err(ViewError::Cycle { field });
            }
            out.push(item);
            current = next(item)?;
        }
        Ok(out)
    }
}

trait HasIndex {
    fn index(&self) -> u32;
}

/// Drop repeats, keeping first occurrences.
fn distinct<V: HasIndex>(items: Vec<V>) -> Vec<V> {
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    items.into_iter().filter(|item| seen.insert(item.index())).collect()
}

macro_rules! resolver {
    ($name:ident, $View:ident, $variant:ident, $label:literal) => {
        impl<'a> Model<'a> {
            fn $name<T>(
                &'a self,
                field: &'static str,
                pointer: XtPointer<T>,
            ) -> Result<Option<$View<'a>>> {
                match self.lookup(field, pointer)? {
                    None => Ok(None),
                    Some(Node::$variant(node)) => Ok(Some($View { model: self, node })),
                    Some(other) => Err(ViewError::WrongNode {
                        field,
                        expected: $label,
                        found: kind_name(other),
                    }),
                }
            }
        }
    };
}

macro_rules! view_type {
    ($View:ident, $node:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy)]
        pub struct $View<'a> {
            model: &'a Model<'a>,
            node: &'a $node,
        }
        impl<'a> $View<'a> {
            /// The underlying generated node.
            pub fn node(&self) -> &'a $node {
                self.node
            }
            pub fn file_index(&self) -> u32 {
                self.node.file_index
            }
        }
        impl HasIndex for $View<'_> {
            fn index(&self) -> u32 {
                self.node.file_index
            }
        }
        impl std::fmt::Debug for $View<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!(stringify!($View), "#{}"), self.node.file_index)
            }
        }
    };
}

view_type!(Body, n::Body, "A body and the entry point to its topology.");
view_type!(Region, n::Region, "A connected part of space bounded by shells.");
view_type!(Shell, n::Shell, "A connected set of faces (or an isolated vertex or wire).");
view_type!(Face, n::Face, "A bounded piece of a surface.");
view_type!(Loop, n::Loop, "A closed boundary circuit of a face.");
view_type!(Fin, n::Halfedge, "One use of an edge by a loop (a halfedge).");
view_type!(Edge, n::Edge, "A bounded piece of a curve shared by its fins.");
view_type!(Vertex, n::Vertex, "A point where edges meet.");

resolver!(region_at, Region, Region, "REGION");
resolver!(shell_at, Shell, Shell, "SHELL");
resolver!(face_at, Face, Face, "FACE");
resolver!(loop_at, Loop, Loop, "LOOP");
resolver!(fin_at, Fin, Halfedge, "HALFEDGE");
resolver!(edge_at, Edge, Edge, "EDGE");
resolver!(vertex_at, Vertex, Vertex, "VERTEX");

impl<'a> Body<'a> {
    pub fn kind(&self) -> Result<BodyKind> {
        BodyKind::from_code(self.node.body_type.0)
    }

    pub fn regions(&self) -> Result<Vec<Region<'a>>> {
        let m = self.model;
        let first = m.region_at("body.region", self.node.region)?;
        m.chain("region.next", first, |r| m.region_at("region.next", r.node.next))
    }

    /// Faces of every shell, each face once (listed under its back shell).
    pub fn faces(&self) -> Result<Vec<Face<'a>>> {
        let mut out = Vec::new();
        for region in self.regions()? {
            for shell in region.shells()? {
                out.extend(shell.faces()?);
            }
        }
        Ok(out)
    }

    /// Every edge: those the body owns (face and sheet boundaries) followed by
    /// the wire edges its shells own. Each edge appears once.
    pub fn edges(&self) -> Result<Vec<Edge<'a>>> {
        let m = self.model;
        let first = m.edge_at("body.edge", self.node.edge)?;
        let mut out = m.chain("edge.next", first, |e| m.edge_at("edge.next", e.node.next))?;
        for region in self.regions()? {
            for shell in region.shells()? {
                out.extend(shell.wire_edges()?);
            }
        }
        Ok(distinct(out))
    }

    /// Every vertex: those the body owns followed by the acorn vertices its
    /// shells own. Each vertex appears once.
    pub fn vertices(&self) -> Result<Vec<Vertex<'a>>> {
        let m = self.model;
        let first = m.vertex_at("body.vertex", self.node.vertex)?;
        let mut out = m.chain("vertex.next", first, |v| m.vertex_at("vertex.next", v.node.next))?;
        for region in self.regions()? {
            for shell in region.shells()? {
                out.extend(shell.acorn_vertex()?);
            }
        }
        Ok(distinct(out))
    }
}

impl<'a> Region<'a> {
    pub fn kind(&self) -> Result<RegionKind> {
        RegionKind::from_char(self.node.r#type.0)
    }

    pub fn shells(&self) -> Result<Vec<Shell<'a>>> {
        let m = self.model;
        let first = m.shell_at("region.shell", self.node.shell)?;
        m.chain("shell.next", first, |s| m.shell_at("shell.next", s.node.next))
    }
}

impl<'a> Shell<'a> {
    /// Faces whose back shell is this shell.
    pub fn faces(&self) -> Result<Vec<Face<'a>>> {
        let m = self.model;
        let first = m.face_at("shell.face", self.node.face)?;
        m.chain("face.next", first, |f| m.face_at("face.next", f.node.next))
    }

    /// Edges that belong to no face: the shell's wire edges.
    pub fn wire_edges(&self) -> Result<Vec<Edge<'a>>> {
        let m = self.model;
        let first = m.edge_at("shell.edge", self.node.edge)?;
        m.chain("edge.next", first, |e| m.edge_at("edge.next", e.node.next))
    }

    /// The vertex of an acorn shell (a point belonging to no edge or face).
    pub fn acorn_vertex(&self) -> Result<Option<Vertex<'a>>> {
        self.model.vertex_at("shell.vertex", self.node.vertex)
    }

    /// Faces whose front shell is this shell.
    pub fn front_faces(&self) -> Result<Vec<Face<'a>>> {
        let m = self.model;
        let first = m.face_at("shell.front_face", self.node.front_face)?;
        m.chain("face.next_front", first, |f| {
            m.face_at("face.next_front", f.node.next_front)
        })
    }
}

impl<'a> Face<'a> {
    /// Orientation of the face normal against the surface normal.
    pub fn sense(&self) -> Result<Sense> {
        Sense::from_char("face.sense", self.node.sense.0)
    }

    pub fn tolerance(&self) -> Option<f64> {
        self.node.tolerance.0
    }

    pub fn back_shell(&self) -> Result<Option<Shell<'a>>> {
        self.model.shell_at("face.shell", self.node.shell)
    }

    pub fn front_shell(&self) -> Result<Option<Shell<'a>>> {
        self.model.shell_at("face.front_shell", self.node.front_shell)
    }

    pub fn loops(&self) -> Result<Vec<Loop<'a>>> {
        let m = self.model;
        let first = m.loop_at("face.loop", self.node.r#loop)?;
        m.chain("loop.next", first, |l| m.loop_at("loop.next", l.node.next))
    }

    /// True when the face normal agrees with the surface normal; `None` without a surface.
    pub fn agrees_with_surface(&self) -> Result<Option<bool>> {
        let face_sense = self.sense()?;
        Ok(self.surface()?.map(|surface| {
            (face_sense == Sense::Forward) == (surface.sense() == Sense::Forward)
        }))
    }

    /// `None` for a face with no surface, a state Parasolid accepts on receive and
    /// reports as missing geometry when checked.
    pub fn surface(&self) -> Result<Option<Surface<'a>>> {
        match self.model.lookup("face.surface", self.node.surface)? {
            None => Ok(None),
            Some(node) => classify_surface("face.surface", node).map(Some),
        }
    }
}

/// What bounds a face along one loop.
#[derive(Debug, Clone)]
pub enum LoopBoundary<'a> {
    /// A closed circuit of fins, in circuit order starting at the loop's first fin.
    Circuit(Vec<Fin<'a>>),
    /// A single vertex lying on the face (no edge).
    IsolatedVertex(Vertex<'a>),
}

impl<'a> Loop<'a> {
    pub fn boundary(&self) -> Result<LoopBoundary<'a>> {
        let m = self.model;
        let first = m.fin_at("loop.halfedge", self.node.halfedge)?;
        let Some(start) = first else {
            return Ok(LoopBoundary::Circuit(Vec::new()));
        };
        if matches!(start.node.edge, XtPointer::Null) {
            let vertex = Model::require("halfedge.vertex", start.end_vertex()?)?;
            return Ok(LoopBoundary::IsolatedVertex(vertex));
        }
        let mut out = vec![start];
        let mut current = start.next()?;
        while current.node.file_index != start.node.file_index {
            if out.len() > m.nodes.len() {
                return Err(ViewError::Cycle {
                    field: "halfedge.forward",
                });
            }
            out.push(current);
            current = current.next()?;
        }
        Ok(LoopBoundary::Circuit(out))
    }
}

impl<'a> Fin<'a> {
    pub fn sense(&self) -> Result<Sense> {
        Sense::from_char("halfedge.sense", self.node.sense.0)
    }

    pub fn edge(&self) -> Result<Edge<'a>> {
        Model::require("halfedge.edge", self.model.edge_at("halfedge.edge", self.node.edge)?)
    }

    /// The vertex at the end of the fin in loop order, absent on a ring edge's fin.
    pub fn end_vertex(&self) -> Result<Option<Vertex<'a>>> {
        self.model.vertex_at("halfedge.vertex", self.node.vertex)
    }

    pub fn next(&self) -> Result<Fin<'a>> {
        Model::require("halfedge.forward", self.model.fin_at("halfedge.forward", self.node.forward)?)
    }

    /// The next fin around the same edge, absent when the edge has one fin.
    pub fn other(&self) -> Result<Option<Fin<'a>>> {
        self.model.fin_at("halfedge.other", self.node.other)
    }

    /// The fin's own curve (a parameter-space or surface-parameter curve), when present.
    /// Trimmed-curve wrappers are looked through.
    pub fn curve(&self) -> Result<Option<Curve<'a>>> {
        self.model.carrier("halfedge.curve", self.node.curve)
    }
}

impl<'a> Edge<'a> {
    /// `None` means the edge is exact.
    pub fn tolerance(&self) -> Option<f64> {
        self.node.tolerance.0
    }

    pub fn curve(&self) -> Result<Option<Curve<'a>>> {
        self.model.carrier("edge.curve", self.node.curve)
    }

    /// The fins of the edge that belong to a loop. The ring of `other` fins also
    /// holds loop-less partner fins (the open side of a sheet edge, wire edges),
    /// which are left out.
    pub fn fins(&self) -> Result<Vec<Fin<'a>>> {
        let m = self.model;
        let first = m.fin_at("edge.halfedge", self.node.halfedge)?;
        let Some(start) = first else {
            return Ok(Vec::new());
        };
        let mut out = vec![start];
        let mut current = start.other()?;
        while let Some(fin) = current {
            if fin.node.file_index == start.node.file_index {
                break;
            }
            if out.len() > m.nodes.len() {
                return Err(ViewError::Cycle { field: "halfedge.other" });
            }
            out.push(fin);
            current = fin.other()?;
        }
        out.retain(|fin| !matches!(fin.node.r#loop, XtPointer::Null));
        Ok(out)
    }
}

impl<'a> Vertex<'a> {
    /// `None` means the vertex is exact.
    pub fn tolerance(&self) -> Option<f64> {
        self.node.tolerance.0
    }

    /// `None` for a vertex with no point (accepted on receive, reported as missing
    /// geometry when checked).
    pub fn position(&self) -> Result<Option<Vec3>> {
        let Some(node) = self.model.lookup("vertex.point", self.node.point)? else {
            return Ok(None);
        };
        match node {
            Node::Point(point) => required_vec("point.pvec", point.pvec).map(Some),
            other => Err(ViewError::WrongNode {
                field: "vertex.point",
                expected: "POINT",
                found: kind_name(other),
            }),
        }
    }
}

/// A surface carrier: plain values for analytic surfaces, the node otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surface<'a> {
    Plane(Plane),
    Cylinder(Cylinder),
    Cone(Cone),
    Sphere(Sphere),
    Torus(Torus),
    Other(OtherSurface<'a>),
}

/// A non-analytic surface (B-spline, offset, swept, spun, blend, ...).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtherSurface<'a> {
    pub kind: SurfaceKind,
    pub sense: Sense,
    pub node: &'a Node,
}

impl Surface<'_> {
    /// Orientation of the surface normal against the parametric normal.
    pub fn sense(&self) -> Sense {
        match self {
            Self::Plane(s) => s.sense,
            Self::Cylinder(s) => s.sense,
            Self::Cone(s) => s.sense,
            Self::Sphere(s) => s.sense,
            Self::Torus(s) => s.sense,
            Self::Other(other) => other.sense,
        }
    }

    pub fn kind(&self) -> SurfaceKind {
        match self {
            Self::Plane(_) => SurfaceKind::Plane,
            Self::Cylinder(_) => SurfaceKind::Cylinder,
            Self::Cone(_) => SurfaceKind::Cone,
            Self::Sphere(_) => SurfaceKind::Sphere,
            Self::Torus(_) => SurfaceKind::Torus,
            Self::Other(other) => other.kind,
        }
    }
}

/// A curve carrier: plain values for lines, circles and ellipses, the node otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve<'a> {
    Line(Line),
    Circle(Circle),
    Ellipse(Ellipse),
    Other(OtherCurve<'a>),
}

/// A non-analytic curve (B-spline, intersection, SP-curve, offset, ...).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtherCurve<'a> {
    pub kind: CurveKind,
    pub sense: Sense,
    pub node: &'a Node,
}

impl Curve<'_> {
    pub fn sense(&self) -> Sense {
        match self {
            Self::Line(c) => c.sense,
            Self::Circle(c) => c.sense,
            Self::Ellipse(c) => c.sense,
            Self::Other(other) => other.sense,
        }
    }

    pub fn kind(&self) -> CurveKind {
        match self {
            Self::Line(_) => CurveKind::Line,
            Self::Circle(_) => CurveKind::Circle,
            Self::Ellipse(_) => CurveKind::Ellipse,
            Self::Other(other) => other.kind,
        }
    }
}

fn optional_vec(value: XtVector) -> Option<Vec3> {
    value.0
}

pub fn classify_surface<'a>(field: &'static str, node: &'a Node) -> Result<Surface<'a>> {
    let other = |kind, sense: u8| {
        Ok(Surface::Other(OtherSurface {
            kind,
            sense: Sense::from_char("surface.sense", sense)?,
            node,
        }))
    };
    match node {
        Node::Plane(s) => Ok(Surface::Plane(Plane {
            origin: required_vec("plane.pvec", s.pvec)?,
            normal: required_vec("plane.normal", s.normal)?,
            x_axis: optional_vec(s.x_axis),
            sense: Sense::from_char("plane.sense", s.sense.0)?,
        })),
        Node::Cylinder(s) => Ok(Surface::Cylinder(Cylinder {
            origin: required_vec("cylinder.pvec", s.pvec)?,
            axis: required_vec("cylinder.axis", s.axis)?,
            radius: required_f64("cylinder.radius", s.radius)?,
            x_axis: optional_vec(s.x_axis),
            sense: Sense::from_char("cylinder.sense", s.sense.0)?,
        })),
        Node::Cone(s) => Ok(Surface::Cone(Cone {
            origin: required_vec("cone.pvec", s.pvec)?,
            axis: required_vec("cone.axis", s.axis)?,
            radius: required_f64("cone.radius", s.radius)?,
            half_angle: required_f64("cone.sin_half_angle", s.sin_half_angle)?
                .atan2(required_f64("cone.cos_half_angle", s.cos_half_angle)?),
            x_axis: optional_vec(s.x_axis),
            sense: Sense::from_char("cone.sense", s.sense.0)?,
        })),
        Node::Sphere(s) => Ok(Surface::Sphere(Sphere {
            centre: required_vec("sphere.centre", s.centre)?,
            radius: required_f64("sphere.radius", s.radius)?,
            axis: optional_vec(s.axis),
            x_axis: optional_vec(s.x_axis),
            sense: Sense::from_char("sphere.sense", s.sense.0)?,
        })),
        Node::Torus(s) => Ok(Surface::Torus(Torus {
            centre: required_vec("torus.centre", s.centre)?,
            axis: required_vec("torus.axis", s.axis)?,
            major_radius: required_f64("torus.major_radius", s.major_radius)?,
            minor_radius: required_f64("torus.minor_radius", s.minor_radius)?,
            x_axis: optional_vec(s.x_axis),
            sense: Sense::from_char("torus.sense", s.sense.0)?,
        })),
        Node::Pipe(s) => other(SurfaceKind::Pipe, s.sense.0),
        Node::BlendedEdge(s) => other(SurfaceKind::BlendedEdge, s.sense.0),
        Node::BlendedVertex(s) => other(SurfaceKind::BlendedVertex, s.sense.0),
        Node::BlendOverlap(s) => other(SurfaceKind::BlendOverlap, s.sense.0),
        Node::BlendBound(s) => other(SurfaceKind::BlendBound, s.sense.0),
        Node::OffsetSurf(s) => other(SurfaceKind::Offset, s.sense.0),
        Node::Parasurf(s) => other(SurfaceKind::Parasurf, s.sense.0),
        Node::ObsoleteCps(s) => other(SurfaceKind::ObsoleteCps, s.sense.0),
        Node::SilhSurf(s) => other(SurfaceKind::Silhouette, s.sense.0),
        Node::SweptSurf(s) => other(SurfaceKind::Swept, s.sense.0),
        Node::SpunSurf(s) => other(SurfaceKind::Spun, s.sense.0),
        Node::Cps(s) => other(SurfaceKind::Cps, s.sense.0),
        Node::ObsoleteMesh(s) => other(SurfaceKind::ObsoleteMesh, s.sense.0),
        Node::PeSurf(s) => other(SurfaceKind::Foreign, s.sense.0),
        Node::BSurface(s) => other(SurfaceKind::Bspline, s.sense.0),
        Node::Mesh(s) => other(SurfaceKind::Mesh, s.sense.0),
        found => Err(ViewError::WrongNode {
            field,
            expected: "a surface",
            found: kind_name(found),
        }),
    }
}

pub fn classify_curve<'a>(field: &'static str, node: &'a Node) -> Result<Curve<'a>> {
    let other = |kind, sense: u8| {
        Ok(Curve::Other(OtherCurve {
            kind,
            sense: Sense::from_char("curve.sense", sense)?,
            node,
        }))
    };
    match node {
        Node::Line(c) => Ok(Curve::Line(Line {
            origin: required_vec("line.pvec", c.pvec)?,
            direction: required_vec("line.direction", c.direction)?,
            sense: Sense::from_char("line.sense", c.sense.0)?,
        })),
        Node::Circle(c) => Ok(Curve::Circle(Circle {
            centre: required_vec("circle.centre", c.centre)?,
            normal: required_vec("circle.normal", c.normal)?,
            x_axis: optional_vec(c.x_axis),
            radius: required_f64("circle.radius", c.radius)?,
            sense: Sense::from_char("circle.sense", c.sense.0)?,
        })),
        Node::Ellipse(c) => Ok(Curve::Ellipse(Ellipse {
            centre: required_vec("ellipse.centre", c.centre)?,
            normal: required_vec("ellipse.normal", c.normal)?,
            x_axis: optional_vec(c.x_axis),
            major_radius: required_f64("ellipse.major_radius", c.major_radius)?,
            minor_radius: required_f64("ellipse.minor_radius", c.minor_radius)?,
            sense: Sense::from_char("ellipse.sense", c.sense.0)?,
        })),
        Node::Parabola(s) => other(CurveKind::Parabola, s.sense.0),
        Node::Hyperbola(s) => other(CurveKind::Hyperbola, s.sense.0),
        Node::Paracurve(s) => other(CurveKind::Paracurve, s.sense.0),
        Node::ObsoleteCpc(s) => other(CurveKind::ObsoleteCpc, s.sense.0),
        Node::PatchBound(s) => other(CurveKind::PatchBound, s.sense.0),
        Node::Intersection(s) => other(CurveKind::Intersection, s.sense.0),
        Node::Silhouette(s) => other(CurveKind::Silhouette, s.sense.0),
        Node::OffsetCurve(s) => other(CurveKind::Offset, s.sense.0),
        Node::Cpc(s) => other(CurveKind::Cpc, s.sense.0),
        Node::ObsoleteSpCurve(s) => other(CurveKind::ObsoleteSpCurve, s.sense.0),
        Node::ObsoletePolyline(s) => other(CurveKind::ObsoletePolyline, s.sense.0),
        Node::PeCurve(s) => other(CurveKind::Foreign, s.sense.0),
        Node::Pcurve(s) => other(CurveKind::Pcurve, s.sense.0),
        Node::TrimmedCurve(s) => other(CurveKind::Trimmed, s.sense.0),
        Node::BCurve(s) => other(CurveKind::Bspline, s.sense.0),
        Node::SpCurve(s) => other(CurveKind::SpCurve, s.sense.0),
        Node::Polyline(s) => other(CurveKind::Polyline, s.sense.0),
        found => Err(ViewError::WrongNode {
            field,
            expected: "a curve",
            found: kind_name(found),
        }),
    }
}
