//! Serialise typed `SCH_37102` nodes to Parasolid XT text.
//!
//! The caller builds typed nodes with explicit file indices (the BODY first, at
//! index 1) and every pointer and chain consistent; the writer converts them to
//! records, checks the rules Parasolid applies on receive, and emits the
//! non-embedded current form (`SCH_<version>_37102`, no schema edits, no
//! container header) that `PK_PART_receive` accepts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::Value;
use xt_parser::schema::{ElementCount, FieldType, NodeSpec};
use xt_parser::write::{SinkError, TextSink};
use xt_parser::{FieldValue, NodeRecord, ScalarValue, Span};

use crate::schemas::sch_37102::{Node, SCHEMA};

/// Column at which output lines are broken.
const LINE_WIDTH: usize = 200;

/// Largest deviation from 1 accepted for the length of a direction vector.
const UNIT_LENGTH_TOLERANCE: f64 = 1e-12;

/// Direction-valued fields by node name; Parasolid does not normalise them.
const UNIT_FIELDS: &[(&str, &[&str])] = &[
    ("PLANE", &["normal", "x_axis"]),
    ("CYLINDER", &["axis", "x_axis"]),
    ("CONE", &["axis", "x_axis"]),
    ("SPHERE", &["axis", "x_axis"]),
    ("TORUS", &["axis", "x_axis"]),
    ("CIRCLE", &["normal", "x_axis"]),
    ("ELLIPSE", &["normal", "x_axis"]),
    ("LINE", &["direction"]),
];

/// Why a node list cannot be written as a body Parasolid will accept.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteError {
    Empty,
    FirstNodeNotBody,
    RootIndexNotOne {
        found: u32,
    },
    ZeroIndex {
        node: String,
    },
    DuplicateIndex {
        index: u32,
    },
    MissingIndex {
        node: u32,
        field: String,
        target: u32,
    },
    NonUnitDirection {
        node: u32,
        field: String,
        length: f64,
    },
    VertexToleranceBelowEdge {
        vertex: u32,
        edge: u32,
        vertex_tolerance: Option<f64>,
        edge_tolerance: f64,
    },
    TolerantEdgeKeepsCurve {
        edge: u32,
    },
    TolerantEdgeFinWithoutCurve {
        edge: u32,
        halfedge: u32,
    },
    ResolutionMismatch {
        field: &'static str,
        found: Option<f64>,
        expected: f64,
    },
    NonFinite {
        node: u32,
        field: String,
    },
    UnsupportedField {
        node: u32,
        field: String,
    },
    Conversion {
        node: u32,
        detail: String,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no nodes to write"),
            Self::FirstNodeNotBody => write!(f, "the first node must be the BODY"),
            Self::RootIndexNotOne { found } => {
                write!(f, "the BODY must have file index 1, got {found}")
            }
            Self::ZeroIndex { node } => write!(f, "{node} has file index 0"),
            Self::DuplicateIndex { index } => write!(f, "file index {index} used twice"),
            Self::MissingIndex {
                node,
                field,
                target,
            } => {
                write!(
                    f,
                    "node {node} field {field} points at absent index {target}"
                )
            }
            Self::NonUnitDirection {
                node,
                field,
                length,
            } => {
                write!(f, "node {node} field {field} has length {length}, not 1")
            }
            Self::VertexToleranceBelowEdge {
                vertex,
                edge,
                vertex_tolerance,
                edge_tolerance,
            } => write!(
                f,
                "vertex {vertex} tolerance {vertex_tolerance:?} is below edge {edge} tolerance {edge_tolerance}"
            ),
            Self::TolerantEdgeKeepsCurve { edge } => {
                write!(f, "tolerant edge {edge} keeps a 3-D curve")
            }
            Self::TolerantEdgeFinWithoutCurve { edge, halfedge } => {
                write!(f, "tolerant edge {edge}: halfedge {halfedge} has no curve")
            }
            Self::ResolutionMismatch {
                field,
                found,
                expected,
            } => write!(f, "BODY {field} is {found:?}, the session has {expected}"),
            Self::NonFinite { node, field } => {
                write!(f, "node {node} field {field} is not finite")
            }
            Self::UnsupportedField { node, field } => {
                write!(f, "node {node} field {field} has no text representation")
            }
            Self::Conversion { node, detail } => write!(f, "node {node}: {detail}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Writer for the non-embedded `SCH_<version>_37102` text form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextWriter {
    schema_version: u32,
    res_size: f64,
    res_linear: f64,
}

impl TextWriter {
    /// `schema_version` is the modeller version in the schema name (3701243 for
    /// transmit version 371); `res_size` and `res_linear` must equal the
    /// receiving session's size box and linear precision.
    pub fn new(schema_version: u32, res_size: f64, res_linear: f64) -> Self {
        Self {
            schema_version,
            res_size,
            res_linear,
        }
    }

    /// Check `nodes` and serialise them.
    pub fn write(&self, nodes: &[Node]) -> Result<String, WriteError> {
        let records = nodes_to_records(nodes)?;
        self.validate(&records)?;
        let mut sink = TextSink::new();
        let banner = format!(
            ": TRANSMIT FILE created by modeller version {}",
            self.schema_version
        );
        sink.write_short_string(banner.as_bytes());
        sink.write_short_string(format!("SCH_{}_37102", self.schema_version).as_bytes());
        sink.write_u8(0);
        for record in &records {
            sink.write_record(record).map_err(|error| match error {
                SinkError::NonFinite { context } => WriteError::NonFinite {
                    node: record.file_index,
                    field: context,
                },
                SinkError::FieldTypeMismatch { field } => WriteError::Conversion {
                    node: record.file_index,
                    detail: format!("field {field} holds values of another type"),
                },
            })?;
        }
        sink.write_terminator();
        let mut text = String::from("T");
        text.push_str(&sink.finish(LINE_WIDTH));
        Ok(text)
    }

    fn validate(&self, records: &[NodeRecord]) -> Result<(), WriteError> {
        let first = records.first().ok_or(WriteError::Empty)?;
        if first.node_name != "BODY" {
            return Err(WriteError::FirstNodeNotBody);
        }
        if first.file_index != 1 {
            return Err(WriteError::RootIndexNotOne {
                found: first.file_index,
            });
        }

        let mut by_index: BTreeMap<u32, &NodeRecord> = BTreeMap::new();
        for record in records {
            if record.file_index == 0 {
                return Err(WriteError::ZeroIndex {
                    node: record.node_name.clone(),
                });
            }
            if by_index.insert(record.file_index, record).is_some() {
                return Err(WriteError::DuplicateIndex {
                    index: record.file_index,
                });
            }
        }
        let present: BTreeSet<u32> = by_index.keys().copied().collect();
        for record in records {
            for field in &record.fields {
                if field.field_type != FieldType::PointerIndex {
                    continue;
                }
                for value in &field.values {
                    if let ScalarValue::PointerIndex(Some(target)) = value {
                        if *target != 0 && !present.contains(target) {
                            return Err(WriteError::MissingIndex {
                                node: record.file_index,
                                field: field.name.clone(),
                                target: *target,
                            });
                        }
                    }
                }
            }
        }

        for (field, expected) in [("res_size", self.res_size), ("res_linear", self.res_linear)] {
            let found = double(first, field);
            if found != Some(expected) {
                return Err(WriteError::ResolutionMismatch {
                    field: if field == "res_size" {
                        "res_size"
                    } else {
                        "res_linear"
                    },
                    found,
                    expected,
                });
            }
        }

        for record in records {
            if let Some((_, fields)) = UNIT_FIELDS
                .iter()
                .find(|(name, _)| *name == record.node_name)
            {
                for name in *fields {
                    if let Some([x, y, z]) = vector(record, name) {
                        let length = (x * x + y * y + z * z).sqrt();
                        if (length - 1.0).abs() > UNIT_LENGTH_TOLERANCE {
                            return Err(WriteError::NonUnitDirection {
                                node: record.file_index,
                                field: (*name).to_owned(),
                                length,
                            });
                        }
                    }
                }
            }
        }

        self.validate_tolerances(records, &by_index)
    }

    fn validate_tolerances(
        &self,
        records: &[NodeRecord],
        by_index: &BTreeMap<u32, &NodeRecord>,
    ) -> Result<(), WriteError> {
        for edge in records.iter().filter(|r| r.node_name == "EDGE") {
            if double(edge, "tolerance").is_some() && pointer(edge, "curve").is_some() {
                return Err(WriteError::TolerantEdgeKeepsCurve {
                    edge: edge.file_index,
                });
            }
        }
        for halfedge in records.iter().filter(|r| r.node_name == "HALFEDGE") {
            let Some(edge) = pointer(halfedge, "edge").and_then(|i| by_index.get(&i)) else {
                continue;
            };
            let Some(edge_tolerance) = double(edge, "tolerance") else {
                continue;
            };
            if pointer(halfedge, "curve").is_none() {
                return Err(WriteError::TolerantEdgeFinWithoutCurve {
                    edge: edge.file_index,
                    halfedge: halfedge.file_index,
                });
            }
            if let Some(vertex) = pointer(halfedge, "vertex").and_then(|i| by_index.get(&i)) {
                let vertex_tolerance = double(vertex, "tolerance");
                if vertex_tolerance.is_none_or(|tolerance| tolerance < edge_tolerance) {
                    return Err(WriteError::VertexToleranceBelowEdge {
                        vertex: vertex.file_index,
                        edge: edge.file_index,
                        vertex_tolerance,
                        edge_tolerance,
                    });
                }
            }
        }
        Ok(())
    }
}

fn field<'a>(record: &'a NodeRecord, name: &str) -> Option<&'a FieldValue> {
    record.fields.iter().find(|field| field.name == name)
}

fn double(record: &NodeRecord, name: &str) -> Option<f64> {
    match field(record, name)?.values.first()? {
        ScalarValue::Double(value) => *value,
        _ => None,
    }
}

fn vector(record: &NodeRecord, name: &str) -> Option<[f64; 3]> {
    match field(record, name)?.values.first()? {
        ScalarValue::Vector(value) => *value,
        _ => None,
    }
}

/// A non-null pointer target.
fn pointer(record: &NodeRecord, name: &str) -> Option<u32> {
    match field(record, name)?.values.first()? {
        ScalarValue::PointerIndex(Some(index)) if *index != 0 => Some(*index),
        _ => None,
    }
}

/// Convert typed nodes to raw records, in schema field order.
pub fn nodes_to_records(nodes: &[Node]) -> Result<Vec<NodeRecord>, WriteError> {
    let specs: BTreeMap<String, &NodeSpec> = SCHEMA
        .nodes
        .iter()
        .map(|spec| (normalise(&spec.name), spec))
        .collect();
    nodes
        .iter()
        .map(|node| node_to_record(node, &specs))
        .collect()
}

fn normalise(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn node_to_record(
    node: &Node,
    specs: &BTreeMap<String, &NodeSpec>,
) -> Result<NodeRecord, WriteError> {
    let value = serde_json::to_value(node).map_err(|error| WriteError::Conversion {
        node: 0,
        detail: error.to_string(),
    })?;
    let variant = value["node"].as_str().unwrap_or_default().to_owned();
    let body = &value["value"];
    let file_index = body["file_index"].as_u64().unwrap_or(0) as u32;
    let fail = |detail: String| WriteError::Conversion {
        node: file_index,
        detail,
    };
    let spec = specs
        .get(&normalise(&variant))
        .ok_or_else(|| fail(format!("no schema node for {variant}")))?;

    let variable_length = if spec.variable {
        Some(
            body["variable_length"]
                .as_u64()
                .ok_or_else(|| fail("variable node has no length".into()))? as usize,
        )
    } else {
        None
    };

    let mut fields = Vec::new();
    for field in spec.fields.iter().filter(|field| field.transmitted) {
        let unsupported = || WriteError::UnsupportedField {
            node: file_index,
            field: field.name.clone(),
        };
        if matches!(field.field_type, FieldType::Opaque | FieldType::Tag) {
            return Err(unsupported());
        }
        let raw = body
            .get(&field.name)
            .ok_or_else(|| fail(format!("typed node lacks field {}", field.name)))?;
        let elements: Vec<&Value> = match field.elements {
            ElementCount::Scalar => vec![raw],
            _ => raw
                .as_array()
                .ok_or_else(|| fail(format!("field {} is not a list", field.name)))?
                .iter()
                .collect(),
        };
        match field.elements {
            ElementCount::Variable if Some(elements.len()) != variable_length => {
                return Err(fail(format!(
                    "field {} has {} elements, count is {:?}",
                    field.name,
                    elements.len(),
                    variable_length
                )));
            }
            ElementCount::Fixed(count) if elements.len() != count as usize => {
                return Err(fail(format!(
                    "field {} has {} elements, expected {count}",
                    field.name,
                    elements.len()
                )));
            }
            _ => {}
        }
        let values = elements
            .into_iter()
            .map(|element| scalar(field.field_type, element))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail(format!("field {} has an unexpected value", field.name)))?;
        fields.push(FieldValue {
            name: field.name.clone(),
            field_type: field.field_type,
            values,
            span: Span::at(0),
        });
    }
    Ok(NodeRecord {
        node_type: spec.node_type,
        node_name: spec.name.clone(),
        file_index,
        variable_length,
        schema_update: None,
        fields,
        span: Span::at(0),
    })
}

fn numbers<const N: usize>(value: &Value) -> Option<Option<[f64; N]>> {
    if value.is_null() {
        return Some(None);
    }
    let items = value.as_array()?;
    let mut out = [0.0; N];
    if items.len() != N {
        return None;
    }
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64()?;
    }
    Some(Some(out))
}

fn optional(value: &Value) -> Option<&Value> {
    (!value.is_null()).then_some(value)
}

fn scalar(field_type: FieldType, value: &Value) -> Option<ScalarValue> {
    Some(match field_type {
        FieldType::UnsignedByte => ScalarValue::UnsignedByte(u8::try_from(value.as_u64()?).ok()?),
        FieldType::Character => ScalarValue::Character(u8::try_from(value.as_u64()?).ok()?),
        FieldType::Logical => ScalarValue::Logical(value.as_bool()?),
        FieldType::ShortInteger => ScalarValue::ShortInteger(match optional(value) {
            Some(v) => Some(i16::try_from(v.as_i64()?).ok()?),
            None => None,
        }),
        FieldType::UnicodeCharacter => ScalarValue::UnicodeCharacter(match optional(value) {
            Some(v) => Some(u16::try_from(v.as_u64()?).ok()?),
            None => None,
        }),
        FieldType::Integer => ScalarValue::Integer(match optional(value) {
            Some(v) => Some(i32::try_from(v.as_i64()?).ok()?),
            None => None,
        }),
        FieldType::PointerIndex => ScalarValue::PointerIndex(match value["kind"].as_str()? {
            "unset" => None,
            "null" => Some(0),
            "index" => Some(u32::try_from(value["index"]["raw"].as_u64()?).ok()?),
            _ => return None,
        }),
        FieldType::Double => ScalarValue::Double(match optional(value) {
            Some(v) => Some(v.as_f64()?),
            None => None,
        }),
        FieldType::Interval => ScalarValue::Interval(numbers::<2>(value)?),
        FieldType::Vector => ScalarValue::Vector(numbers::<3>(value)?),
        FieldType::Box => ScalarValue::Box(numbers::<6>(value)?),
        FieldType::IntersectionPoint => ScalarValue::IntersectionPoint(numbers::<3>(value)?),
        FieldType::Opaque | FieldType::Tag => return None,
    })
}
