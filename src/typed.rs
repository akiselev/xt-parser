use std::marker::PhantomData;

use serde::Serialize;

use crate::{ErrorKind, FieldValue, NodeRecord, ScalarValue, Span, XtError};

pub trait TypedNode: Sized {
    const NODE_TYPE: u16;
    const NODE_NAME: &'static str;

    fn try_from_record(record: &NodeRecord) -> Result<Self, XtError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct NodeIndex<T> {
    raw: u32,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> NodeIndex<T> {
    pub fn new(raw: u32) -> Option<Self> {
        (raw != 0).then_some(Self {
            raw,
            marker: PhantomData,
        })
    }

    pub const fn get(self) -> u32 {
        self.raw
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "index", rename_all = "snake_case")]
pub enum XtPointer<T> {
    Null,
    Unset,
    Index(NodeIndex<T>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtByte(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtChar(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtLogical(pub bool);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtShort(pub Option<i16>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtUnicode(pub Option<u16>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtInteger(pub Option<i32>);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct XtDouble(pub Option<f64>);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct XtInterval(pub Option<[f64; 2]>);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct XtVector(pub Option<[f64; 3]>);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct XtBox(pub Option<[f64; 6]>);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct XtIntersectionPoint(pub Option<[f64; 3]>);

/// Typed placeholder for schema types that the native stream decoder has not
/// yet observed (`q` and `t`). Generated APIs retain these fields explicitly
/// instead of inventing a representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct XtUnsupported;

/// Marker used when a schema pointer has no narrower declared class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct AnyNode;

/// Marker for a pointer constrained to one exact schema node type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ExactNode<const NODE_TYPE: u16>;

impl<const NODE_TYPE: u16> ExactNode<NODE_TYPE> {
    pub const NODE_TYPE: u16 = NODE_TYPE;
}

/// Marker for a pointer constrained to an abstract schema pointer class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct AbstractClass<const POINTER_CLASS: u16>;

impl<const POINTER_CLASS: u16> AbstractClass<POINTER_CLASS> {
    pub const POINTER_CLASS: u16 = POINTER_CLASS;
}

#[doc(hidden)]
pub trait FromScalarValue: Sized {
    fn from_scalar(value: &ScalarValue) -> Option<Self>;
}

impl FromScalarValue for XtByte {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::UnsignedByte(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtChar {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Character(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtLogical {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Logical(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtShort {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::ShortInteger(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtUnicode {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::UnicodeCharacter(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtInteger {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Integer(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtDouble {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Double(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtInterval {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Interval(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtVector {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Vector(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtBox {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Box(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl FromScalarValue for XtIntersectionPoint {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::IntersectionPoint(value) => Some(Self(*value)),
            _ => None,
        }
    }
}

impl<T> FromScalarValue for XtPointer<T> {
    fn from_scalar(value: &ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::PointerIndex(None) => Some(Self::Unset),
            ScalarValue::PointerIndex(Some(0)) => Some(Self::Null),
            ScalarValue::PointerIndex(Some(index)) => Some(Self::Index(
                NodeIndex::new(*index).expect("checked nonzero"),
            )),
            _ => None,
        }
    }
}

#[doc(hidden)]
pub struct TypedFields<'a> {
    record: &'a NodeRecord,
    position: usize,
}

impl<'a> TypedFields<'a> {
    pub fn new(record: &'a NodeRecord, node_type: u16, node_name: &str) -> Result<Self, XtError> {
        if record.node_type != node_type {
            return Err(typed_error(
                record.span,
                format!(
                    "expected node {node_type} {node_name}, got {} {}",
                    record.node_type, record.node_name
                ),
            ));
        }
        Ok(Self {
            record,
            position: 0,
        })
    }

    pub fn next_scalar<T: FromScalarValue>(&mut self, name: &str) -> Result<T, XtError> {
        let field = self.next_field(name)?;
        if field.values.len() != 1 {
            return Err(typed_error(
                field.span,
                format!(
                    "field {name} has {} values, expected scalar",
                    field.values.len()
                ),
            ));
        }
        T::from_scalar(&field.values[0]).ok_or_else(|| {
            typed_error(
                field.span,
                format!("field {name} has an unexpected scalar representation"),
            )
        })
    }

    pub fn next_fixed<T: FromScalarValue, const N: usize>(
        &mut self,
        name: &str,
    ) -> Result<[T; N], XtError> {
        let values = self.convert_values::<T>(name)?;
        values.try_into().map_err(|values: Vec<T>| {
            typed_error(
                self.record.span,
                format!("field {name} has {} values, expected {N}", values.len()),
            )
        })
    }

    pub fn next_variable<T: FromScalarValue>(&mut self, name: &str) -> Result<Vec<T>, XtError> {
        self.convert_values(name)
    }

    pub fn next_unsupported(&mut self, name: &str) -> Result<XtUnsupported, XtError> {
        let field = self.next_field(name)?;
        Err(typed_error(
            field.span,
            format!("field {name} uses an unsupported generated schema type"),
        ))
    }

    pub fn next_unsupported_fixed<const N: usize>(
        &mut self,
        name: &str,
    ) -> Result<[XtUnsupported; N], XtError> {
        let field = self.next_field(name)?;
        Err(typed_error(
            field.span,
            format!("field {name} uses an unsupported generated schema type"),
        ))
    }

    pub fn next_unsupported_variable(&mut self, name: &str) -> Result<Vec<XtUnsupported>, XtError> {
        let field = self.next_field(name)?;
        Err(typed_error(
            field.span,
            format!("field {name} uses an unsupported generated schema type"),
        ))
    }

    pub fn finish(self) -> Result<(), XtError> {
        if self.position == self.record.fields.len() {
            Ok(())
        } else {
            Err(typed_error(
                self.record.fields[self.position].span,
                format!(
                    "{} unconsumed typed fields remain",
                    self.record.fields.len() - self.position
                ),
            ))
        }
    }

    fn convert_values<T: FromScalarValue>(&mut self, name: &str) -> Result<Vec<T>, XtError> {
        let field = self.next_field(name)?;
        field
            .values
            .iter()
            .map(|value| {
                T::from_scalar(value).ok_or_else(|| {
                    typed_error(
                        field.span,
                        format!("field {name} has an unexpected scalar representation"),
                    )
                })
            })
            .collect()
    }

    fn next_field(&mut self, name: &str) -> Result<&'a FieldValue, XtError> {
        let field = self.record.fields.get(self.position).ok_or_else(|| {
            typed_error(self.record.span, format!("missing generated field {name}"))
        })?;
        if field.name != name {
            return Err(typed_error(
                field.span,
                format!(
                    "generated field order expected {name}, raw record contains {}",
                    field.name
                ),
            ));
        }
        self.position += 1;
        Ok(field)
    }
}

fn typed_error(span: Span, context: impl Into<String>) -> XtError {
    XtError::new(ErrorKind::InvalidNode, span, context)
}
