//! Tests of the resolved view on a hand-built triangular sheet body.

use xt_parser::{
    NodeIndex, NodeRecord, Span, XtByte, XtChar, XtDouble, XtInteger, XtPointer, XtVector,
};

use super::{
    BodyKind, RegionKind, Sense, ViewError,
    sch_30100::{Curve, LoopBoundary, Model, Surface, classify_curve, classify_surface},
};
use crate::schemas::sch_30100::{Node, nodes as n};

fn ptr<T>(index: u32) -> XtPointer<T> {
    XtPointer::Index(NodeIndex::new(index).expect("nonzero"))
}

fn null<T>() -> XtPointer<T> {
    XtPointer::Null
}

fn int(value: i32) -> XtInteger {
    XtInteger(Some(value))
}

fn vec3(x: f64, y: f64, z: f64) -> XtVector {
    XtVector(Some([x, y, z]))
}

const BODY: u32 = 1;
const REGION_VOID: u32 = 2;
const REGION_SOLID: u32 = 3;
const SHELL: u32 = 4;
const FACE: u32 = 5;
const LOOP: u32 = 6;
const FIN: [u32; 3] = [7, 8, 9];
const EDGE: [u32; 3] = [10, 11, 12];
const VERTEX: [u32; 3] = [13, 14, 15];
const POINT: [u32; 3] = [16, 17, 18];
const PLANE: u32 = 19;
const LINE: [u32; 3] = [20, 21, 22];

const CORNERS: [[f64; 3]; 3] = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]];

/// A sheet body: one planar triangle face on a void region, plus an empty solid region.
fn fixture() -> Vec<Node> {
    let mut nodes = Vec::new();
    nodes.push(Node::Body(n::Body {
        file_index: BODY,
        highest_node_id: int(22),
        attributes_features: null(),
        attribute_chains: null(),
        surface: null(),
        curve: null(),
        point: null(),
        mesh: null(),
        polyline: null(),
        key: null(),
        res_size: XtDouble(None),
        res_linear: XtDouble(None),
        ref_instance: null(),
        next: null(),
        previous: null(),
        state: XtByte(0),
        owner: null(),
        body_type: XtByte(3),
        nom_geom_state: XtByte(0),
        shell: null(),
        boundary_surface: null(),
        boundary_curve: null(),
        boundary_point: null(),
        boundary_mesh: null(),
        boundary_polyline: null(),
        region: ptr(REGION_VOID),
        edge: ptr(EDGE[0]),
        vertex: ptr(VERTEX[0]),
        index_map_offset: int(0),
        index_map: null(),
        node_id_index_map: null(),
        schema_embedding_map: null(),
        child: null(),
        lowest_node_id: int(1),
        mesh_offset_data: null(),
    }));
    for (index, kind, shell, next) in [
        (REGION_VOID, b'V', ptr(SHELL), ptr(REGION_SOLID)),
        (REGION_SOLID, b'S', null(), null()),
    ] {
        nodes.push(Node::Region(n::Region {
            file_index: index,
            node_id: int(index as i32),
            attributes_features: null(),
            body: ptr(BODY),
            next,
            previous: null(),
            shell,
            r#type: XtChar(kind),
            owner: ptr(BODY),
        }));
    }
    nodes.push(Node::Shell(n::Shell {
        file_index: SHELL,
        node_id: int(4),
        attributes_features: null(),
        body: ptr(BODY),
        next: null(),
        face: ptr(FACE),
        edge: null(),
        vertex: null(),
        region: ptr(REGION_VOID),
        front_face: null(),
    }));
    nodes.push(Node::Face(n::Face {
        file_index: FACE,
        node_id: int(5),
        attributes_features: null(),
        tolerance: XtDouble(None),
        next: null(),
        previous: null(),
        r#loop: ptr(LOOP),
        shell: ptr(SHELL),
        surface: ptr(PLANE),
        sense: XtChar(b'+'),
        next_on_surface: null(),
        previous_on_surface: null(),
        next_front: null(),
        previous_front: null(),
        front_shell: null(),
    }));
    nodes.push(Node::Loop(n::Loop {
        file_index: LOOP,
        node_id: int(6),
        attributes_features: null(),
        halfedge: ptr(FIN[0]),
        face: ptr(FACE),
        next: null(),
    }));
    for i in 0..3 {
        nodes.push(Node::Halfedge(n::Halfedge {
            file_index: FIN[i],
            attributes_features: null(),
            r#loop: ptr(LOOP),
            forward: ptr(FIN[(i + 1) % 3]),
            backward: ptr(FIN[(i + 2) % 3]),
            vertex: ptr(VERTEX[i]),
            other: null(),
            edge: ptr(EDGE[i]),
            curve: null(),
            next_at_vx: null(),
            sense: XtChar(b'+'),
        }));
    }
    for i in 0..3 {
        nodes.push(Node::Edge(n::Edge {
            file_index: EDGE[i],
            node_id: int(EDGE[i] as i32),
            attributes_features: null(),
            tolerance: XtDouble((i == 0).then_some(1e-6)),
            halfedge: ptr(FIN[i]),
            previous: null(),
            next: if i < 2 { ptr(EDGE[i + 1]) } else { null() },
            curve: ptr(LINE[i]),
            next_on_curve: null(),
            previous_on_curve: null(),
            owner: ptr(BODY),
        }));
    }
    for i in 0..3 {
        nodes.push(Node::Vertex(n::Vertex {
            file_index: VERTEX[i],
            node_id: int(VERTEX[i] as i32),
            attributes_features: null(),
            halfedge: ptr(FIN[i]),
            previous: null(),
            next: if i < 2 { ptr(VERTEX[i + 1]) } else { null() },
            point: ptr(POINT[i]),
            tolerance: XtDouble(None),
            owner: ptr(BODY),
        }));
    }
    for i in 0..3 {
        let [x, y, z] = CORNERS[i];
        nodes.push(Node::Point(n::Point {
            file_index: POINT[i],
            node_id: int(POINT[i] as i32),
            attributes_features: null(),
            owner: null(),
            next: null(),
            previous: null(),
            pvec: vec3(x, y, z),
        }));
    }
    nodes.push(Node::Plane(n::Plane {
        file_index: PLANE,
        node_id: int(19),
        attributes_features: null(),
        owner: null(),
        next: null(),
        previous: null(),
        geometric_owner: null(),
        sense: XtChar(b'+'),
        pvec: vec3(0.0, 0.0, 1.0),
        normal: vec3(0.0, 0.0, 1.0),
        x_axis: XtVector(None),
    }));
    for i in 0..3 {
        let [x, y, z] = CORNERS[i];
        nodes.push(Node::Line(n::Line {
            file_index: LINE[i],
            node_id: int(LINE[i] as i32),
            attributes_features: null(),
            owner: null(),
            next: null(),
            previous: null(),
            geometric_owner: null(),
            sense: XtChar(b'+'),
            pvec: vec3(x, y, z),
            direction: vec3(1.0, 0.0, 0.0),
        }));
    }
    nodes
}

fn records(nodes: &[Node]) -> Vec<NodeRecord> {
    let index = |node: &Node| match node {
        Node::Body(x) => x.file_index,
        Node::Region(x) => x.file_index,
        Node::Shell(x) => x.file_index,
        Node::Face(x) => x.file_index,
        Node::Loop(x) => x.file_index,
        Node::Halfedge(x) => x.file_index,
        Node::Edge(x) => x.file_index,
        Node::Vertex(x) => x.file_index,
        Node::Point(x) => x.file_index,
        Node::Plane(x) => x.file_index,
        Node::Line(x) => x.file_index,
        _ => unreachable!("fixture contains only these nodes"),
    };
    nodes
        .iter()
        .map(|node| NodeRecord {
            node_type: 0,
            node_name: String::new(),
            file_index: index(node),
            variable_length: None,
            schema_update: None,
            fields: Vec::new(),
            span: Span::at(0),
        })
        .collect()
}

fn with_model<R>(nodes: Vec<Node>, check: impl FnOnce(&Model<'_>) -> R) -> R {
    let records = records(&nodes);
    let model = Model::new(&records, &nodes).expect("model");
    check(&model)
}

#[test]
fn walks_regions_shells_faces_loops_and_fins() {
    with_model(fixture(), |model| {
        let body = model.bodies().next().expect("one body");
        assert_eq!(body.kind().unwrap(), BodyKind::Sheet);
        let regions = body.regions().unwrap();
        let kinds: Vec<_> = regions.iter().map(|r| r.kind().unwrap()).collect();
        assert_eq!(kinds, [RegionKind::Void, RegionKind::Solid]);
        assert_eq!(regions[0].shells().unwrap().len(), 1);
        assert_eq!(regions[1].shells().unwrap().len(), 0);

        let faces = body.faces().unwrap();
        assert_eq!(faces.len(), 1);
        let face = faces[0];
        assert_eq!(face.sense().unwrap(), Sense::Forward);
        assert_eq!(face.back_shell().unwrap().unwrap().file_index(), SHELL);
        assert!(face.front_shell().unwrap().is_none());

        let loops = face.loops().unwrap();
        assert_eq!(loops.len(), 1);
        let LoopBoundary::Circuit(fins) = loops[0].boundary().unwrap() else {
            panic!("expected a circuit");
        };
        let indices: Vec<_> = fins.iter().map(|f| f.file_index()).collect();
        assert_eq!(indices, FIN);
        for (fin, edge) in fins.iter().zip(EDGE) {
            assert_eq!(fin.edge().unwrap().file_index(), edge);
            assert_eq!(fin.sense().unwrap(), Sense::Forward);
            assert!(fin.other().unwrap().is_none());
            assert_eq!(fin.next().unwrap().file_index(), FIN[(fin.file_index() as usize - 7 + 1) % 3]);
        }
    });
}

#[test]
fn a_loop_without_an_edge_is_an_isolated_vertex() {
    let mut nodes = fixture();
    for node in &mut nodes {
        if let Node::Halfedge(fin) = node {
            if fin.file_index == FIN[0] {
                fin.edge = null();
                fin.forward = ptr(FIN[0]);
                fin.backward = ptr(FIN[0]);
            }
        }
    }
    with_model(nodes, |model| {
        let face = model.bodies().next().unwrap().faces().unwrap()[0];
        match face.loops().unwrap()[0].boundary().unwrap() {
            LoopBoundary::IsolatedVertex(vertex) => assert_eq!(vertex.file_index(), VERTEX[0]),
            other => panic!("{other:?}"),
        }
    });
}

#[test]
fn edges_vertices_and_tolerances() {
    with_model(fixture(), |model| {
        let body = model.bodies().next().unwrap();
        let edges = body.edges().unwrap();
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].tolerance(), Some(1e-6));
        assert_eq!(edges[1].tolerance(), None);
        assert_eq!(edges[2].fins().unwrap().len(), 1);
        match edges[1].curve().unwrap().unwrap() {
            Curve::Line(line) => {
                assert_eq!(line.origin, CORNERS[1]);
                assert_eq!(line.direction, [1.0, 0.0, 0.0]);
            }
            other => panic!("{other:?}"),
        }
        let positions: Vec<_> = body
            .vertices()
            .unwrap()
            .iter()
            .map(|v| v.position().unwrap())
            .collect();
        assert_eq!(positions, CORNERS);
        assert_eq!(body.vertices().unwrap()[0].tolerance(), None);
    });
}

#[test]
fn face_surface_is_a_plain_plane() {
    with_model(fixture(), |model| {
        let face = model.bodies().next().unwrap().faces().unwrap()[0];
        match face.surface().unwrap() {
            Surface::Plane(plane) => {
                assert_eq!(plane.origin, [0.0, 0.0, 1.0]);
                assert_eq!(plane.normal, [0.0, 0.0, 1.0]);
                assert_eq!(plane.x_axis, None);
                assert_eq!(plane.sense, Sense::Forward);
            }
            other => panic!("{other:?}"),
        }
    });
}

#[test]
fn a_ring_of_other_fins_is_walked_once() {
    let mut nodes = fixture();
    // Make fin 7 and fin 8 share edge 10 as a two-fin ring.
    for node in &mut nodes {
        if let Node::Halfedge(fin) = node {
            match fin.file_index {
                7 => fin.other = ptr(8),
                8 => {
                    fin.other = ptr(7);
                    fin.edge = ptr(EDGE[0]);
                }
                _ => {}
            }
        }
    }
    with_model(nodes, |model| {
        let edge = model.bodies().next().unwrap().edges().unwrap()[0];
        let fins: Vec<_> = edge.fins().unwrap().iter().map(|f| f.file_index()).collect();
        assert_eq!(fins, [7, 8]);
    });
}

#[test]
fn a_loopless_partner_fin_is_not_counted() {
    let mut nodes = fixture();
    for node in &mut nodes {
        if let Node::Halfedge(fin) = node {
            match fin.file_index {
                7 => fin.other = ptr(8),
                8 => {
                    fin.other = ptr(7);
                    fin.edge = ptr(EDGE[0]);
                    fin.r#loop = null();
                }
                _ => {}
            }
        }
    }
    with_model(nodes, |model| {
        let edge = model.bodies().next().unwrap().edges().unwrap()[0];
        let fins: Vec<_> = edge.fins().unwrap().iter().map(|f| f.file_index()).collect();
        assert_eq!(fins, [7]);
    });
}

#[test]
fn wire_edges_and_isolated_vertices_come_from_the_shell() {
    let mut nodes = fixture();
    for node in &mut nodes {
        match node {
            Node::Body(body) => {
                body.edge = null();
                body.vertex = null();
            }
            Node::Shell(shell) => {
                shell.edge = ptr(EDGE[0]);
                shell.vertex = ptr(VERTEX[0]);
            }
            _ => {}
        }
    }
    with_model(nodes, |model| {
        let body = model.bodies().next().unwrap();
        let edges: Vec<_> = body.edges().unwrap().iter().map(|e| e.file_index()).collect();
        assert_eq!(edges, EDGE);
        let vertices: Vec<_> = body.vertices().unwrap().iter().map(|v| v.file_index()).collect();
        assert_eq!(vertices, VERTEX);
    });
}

#[test]
fn cone_half_angle_comes_from_sine_and_cosine() {
    let node = Node::Cone(n::Cone {
        file_index: 1,
        node_id: int(1),
        attributes_features: null(),
        owner: null(),
        next: null(),
        previous: null(),
        geometric_owner: null(),
        sense: XtChar(b'-'),
        pvec: vec3(1.0, 2.0, 3.0),
        axis: vec3(0.0, 0.0, 1.0),
        radius: XtDouble(Some(4.0)),
        sin_half_angle: XtDouble(Some(0.5)),
        cos_half_angle: XtDouble(Some(3f64.sqrt() / 2.0)),
        x_axis: vec3(1.0, 0.0, 0.0),
    });
    match classify_surface("test", &node).unwrap() {
        Surface::Cone(cone) => {
            assert!((cone.half_angle - std::f64::consts::FRAC_PI_6).abs() < 1e-15);
            assert_eq!(cone.radius, 4.0);
            assert_eq!(cone.sense, Sense::Reversed);
            assert_eq!(cone.x_axis, Some([1.0, 0.0, 0.0]));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_node_that_is_not_a_curve_is_rejected() {
    let nodes = fixture();
    let plane = nodes.iter().find(|n| matches!(n, Node::Plane(_))).unwrap();
    assert!(matches!(
        classify_curve("test", plane),
        Err(ViewError::WrongNode { .. })
    ));
}

#[test]
fn malformed_graphs_fail_with_the_field_named() {
    let broken = |edit: &dyn Fn(&mut Node)| {
        let mut nodes = fixture();
        nodes.iter_mut().for_each(edit);
        nodes
    };

    let dangling = broken(&|node| {
        if let Node::Face(face) = node {
            face.r#loop = ptr(99);
        }
    });
    with_model(dangling, |model| {
        let face = model.bodies().next().unwrap().faces().unwrap()[0];
        assert_eq!(
            face.loops().unwrap_err(),
            ViewError::DanglingPointer { field: "face.loop", index: 99 }
        );
    });

    let wrong = broken(&|node| {
        if let Node::Face(face) = node {
            face.shell = ptr(PLANE);
        }
    });
    with_model(wrong, |model| {
        let face = model.bodies().next().unwrap().faces().unwrap()[0];
        assert!(matches!(
            face.back_shell(),
            Err(ViewError::WrongNode { field: "face.shell", .. })
        ));
    });

    let cyclic = broken(&|node| {
        if let Node::Region(region) = node {
            region.next = ptr(region.file_index);
        }
    });
    with_model(cyclic, |model| {
        let body = model.bodies().next().unwrap();
        assert_eq!(body.regions().unwrap_err(), ViewError::Cycle { field: "region.next" });
    });

    let unset = broken(&|node| {
        if let Node::Edge(edge) = node {
            edge.curve = XtPointer::Unset;
        }
    });
    with_model(unset, |model| {
        let edge = model.bodies().next().unwrap().edges().unwrap()[0];
        assert_eq!(edge.curve().unwrap_err(), ViewError::UnsetPointer { field: "edge.curve" });
    });

    let bad_sense = broken(&|node| {
        if let Node::Face(face) = node {
            face.sense = XtChar(b'x');
        }
    });
    with_model(bad_sense, |model| {
        let face = model.bodies().next().unwrap().faces().unwrap()[0];
        assert_eq!(
            face.sense().unwrap_err(),
            ViewError::BadEnumeration { field: "face.sense", value: b'x' }
        );
    });
}

#[test]
fn duplicate_file_indices_are_rejected() {
    let nodes = fixture();
    let mut records = records(&nodes);
    records[1].file_index = records[0].file_index;
    assert_eq!(
        Model::new(&records, &nodes).unwrap_err(),
        ViewError::DuplicateIndex { index: 1 }
    );
}
