use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{
    BinaryEnvelope, ErrorKind, MachineEncoding, Result, Span, TextCursor, TextEnvelope, XtError,
    binary::BinaryCursor,
    schema::{ElementCount, FieldEdit, FieldSpec, FieldType, NodeSpec, Schema, apply_field_edits},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaUpdateKind {
    BaseMatch,
    Edited,
    NewNode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ScalarValue {
    UnsignedByte(u8),
    Character(u8),
    Logical(bool),
    ShortInteger(Option<i16>),
    UnicodeCharacter(Option<u16>),
    Integer(Option<i32>),
    PointerIndex(Option<u32>),
    Double(Option<f64>),
    Interval(Option<[f64; 2]>),
    Vector(Option<[f64; 3]>),
    Box(Option<[f64; 6]>),
    IntersectionPoint(Option<[f64; 3]>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldValue {
    pub name: String,
    pub field_type: FieldType,
    pub values: Vec<ScalarValue>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NodeRecord {
    pub node_type: u16,
    pub node_name: String,
    pub file_index: u32,
    pub variable_length: Option<usize>,
    pub schema_update: Option<SchemaUpdateKind>,
    pub fields: Vec<FieldValue>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Terminator {
    pub short_value: i16,
    pub index: u32,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnresolvedPointer {
    pub source_file_index: u32,
    pub source_node_type: u16,
    pub field_name: String,
    pub element_index: usize,
    pub target_index: u32,
}

#[derive(Debug, Clone)]
pub struct XtDocument {
    pub envelope: TextEnvelope,
    pub nodes: Vec<NodeRecord>,
    pub terminator: Terminator,
    pub unresolved_pointer_indices: BTreeSet<u32>,
    pub unresolved_pointers: Vec<UnresolvedPointer>,
}

#[derive(Debug, Clone)]
pub struct BinaryXtDocument {
    pub envelope: BinaryEnvelope,
    pub nodes: Vec<NodeRecord>,
    pub terminator: Terminator,
    pub unresolved_pointer_indices: BTreeSet<u32>,
    pub unresolved_pointers: Vec<UnresolvedPointer>,
}

struct ParsedGraph {
    nodes: Vec<NodeRecord>,
    terminator: Terminator,
    unresolved_pointer_indices: BTreeSet<u32>,
    unresolved_pointers: Vec<UnresolvedPointer>,
}

impl XtDocument {
    pub fn parse_text(input: &[u8], base: &Schema, target: &Schema) -> Result<Self> {
        let envelope = TextEnvelope::parse(input)?;
        validate_schema_selection(
            &envelope.prefix.schema_name,
            envelope.prefix.max_node_types,
            envelope.prefix.span,
            base,
            target,
        )?;
        let mut cursor = envelope.stream.cursor_at(envelope.node_start)?;
        let graph = parse_graph(&mut cursor, base, target)?;
        Ok(Self {
            envelope,
            nodes: graph.nodes,
            terminator: graph.terminator,
            unresolved_pointer_indices: graph.unresolved_pointer_indices,
            unresolved_pointers: graph.unresolved_pointers,
        })
    }
}

impl BinaryXtDocument {
    pub fn parse(input: &[u8], base: &Schema, target: &Schema) -> Result<Self> {
        Self::parse_with_bare_machine(input, base, target, None)
    }

    pub fn parse_with_bare_machine(
        input: &[u8],
        base: &Schema,
        target: &Schema,
        bare_machine: Option<MachineEncoding>,
    ) -> Result<Self> {
        let envelope = BinaryEnvelope::parse_with_bare_machine(input, bare_machine)?;
        validate_schema_selection(
            &envelope.prefix.schema_name,
            envelope.prefix.max_node_types,
            envelope.prefix.span,
            base,
            target,
        )?;
        let mut cursor = envelope.cursor(input)?;
        let graph = parse_graph(&mut cursor, base, target)?;
        Ok(Self {
            envelope,
            nodes: graph.nodes,
            terminator: graph.terminator,
            unresolved_pointer_indices: graph.unresolved_pointer_indices,
            unresolved_pointers: graph.unresolved_pointers,
        })
    }
}

fn parse_graph<C: DocumentCursor>(
    cursor: &mut C,
    base: &Schema,
    target: &Schema,
) -> Result<ParsedGraph> {
    let mut layouts: BTreeMap<u16, Vec<FieldSpec>> = BTreeMap::new();
    let mut file_indices = BTreeSet::new();
    let mut nodes = Vec::new();

    let terminator = loop {
        let record_start = cursor.position();
        let node_type_value = cursor.read_i16()?.ok_or_else(|| {
            node_error(cursor.span_from(record_start), "node type cannot be unset")
        })?;
        let node_type = u16::try_from(node_type_value).map_err(|_| {
            node_error(
                cursor.span_from(record_start),
                "node type must not be negative",
            )
        })?;

        if node_type == 1 {
            let index = cursor.read_final_index()?.ok_or_else(|| {
                node_error(
                    cursor.span_from(record_start),
                    "terminator index cannot be unset",
                )
            })?;
            if index != 0 {
                return Err(node_error(
                    cursor.span_from(record_start),
                    format!("terminator index must be 0, got {index}"),
                ));
            }
            break Terminator {
                short_value: 1,
                index,
                span: cursor.span_from(record_start),
            };
        }

        let target_node = target.node(node_type).ok_or_else(|| {
            node_error(
                cursor.span_from(record_start),
                format!("node type {node_type} is absent from target schema"),
            )
        })?;

        let schema_update = if let std::collections::btree_map::Entry::Vacant(entry) =
            layouts.entry(node_type)
        {
            let (layout, update) = read_first_node_schema(cursor, node_type, base, target_node)?;
            entry.insert(layout);
            Some(update)
        } else {
            None
        };

        let variable_length = if target_node.variable {
            let value = cursor.read_i32()?.ok_or_else(|| {
                node_error(
                    cursor.span_from(record_start),
                    "variable-length count cannot be unset",
                )
            })?;
            Some(usize::try_from(value).map_err(|_| {
                node_error(
                    cursor.span_from(record_start),
                    "variable-length count must not be negative",
                )
            })?)
        } else {
            None
        };

        let file_index = cursor.read_index()?.ok_or_else(|| {
            node_error(
                cursor.span_from(record_start),
                "node file index cannot be unset",
            )
        })?;
        if file_index == 0 {
            return Err(node_error(
                cursor.span_from(record_start),
                "ordinary node file index must not be zero",
            ));
        }
        if !file_indices.insert(file_index) {
            return Err(XtError::new(
                ErrorKind::DuplicateNodeIndex,
                cursor.span_from(record_start),
                format!("duplicate node file index {file_index}"),
            ));
        }
        if nodes.is_empty() && file_index != 1 {
            return Err(node_error(
                cursor.span_from(record_start),
                format!("root node must have file index 1, got {file_index}"),
            ));
        }

        let fields = read_node_fields(cursor, target_node, variable_length)?;
        nodes.push(NodeRecord {
            node_type,
            node_name: target_node.name.clone(),
            file_index,
            variable_length,
            schema_update,
            fields,
            span: cursor.span_from(record_start),
        });
    };

    if cursor.remaining() != 0 {
        return Err(XtError::new(
            ErrorKind::TrailingData,
            cursor.span_from(cursor.position()),
            format!(
                "{} logical bytes remain after the terminator",
                cursor.remaining()
            ),
        ));
    }
    if nodes.is_empty() {
        return Err(node_error(terminator.span, "XT document has no root node"));
    }

    let mut unresolved_pointer_indices = BTreeSet::new();
    let mut unresolved_pointers = Vec::new();
    for node in &nodes {
        for field in &node.fields {
            for (element_index, value) in field.values.iter().enumerate() {
                let ScalarValue::PointerIndex(Some(target_index)) = value else {
                    continue;
                };
                if *target_index != 0 && !file_indices.contains(target_index) {
                    unresolved_pointer_indices.insert(*target_index);
                    unresolved_pointers.push(UnresolvedPointer {
                        source_file_index: node.file_index,
                        source_node_type: node.node_type,
                        field_name: field.name.clone(),
                        element_index,
                        target_index: *target_index,
                    });
                }
            }
        }
    }

    Ok(ParsedGraph {
        nodes,
        terminator,
        unresolved_pointer_indices,
        unresolved_pointers,
    })
}

fn validate_schema_selection(
    schema_name: &[u8],
    max_node_types: Option<u16>,
    span: Span,
    base: &Schema,
    target: &Schema,
) -> Result<()> {
    let identity = SchemaIdentity::parse(schema_name).ok_or_else(|| {
        XtError::new(
            ErrorKind::InvalidSchema,
            span,
            "internal schema name is not SCH_<version>_<schema>_<base>",
        )
    })?;
    let expected_base = format!("SCH_{}", identity.base);
    let expected_target = format!("SCH_{}", identity.schema);
    if base.key() != expected_base {
        return Err(XtError::new(
            ErrorKind::InvalidSchema,
            span,
            format!("base schema must be {expected_base}"),
        ));
    }
    if target.key() != expected_target {
        return Err(XtError::new(
            ErrorKind::InvalidSchema,
            span,
            format!("target schema must be {expected_target}"),
        ));
    }
    if max_node_types != Some(target.counts.node_type_limit) {
        return Err(XtError::new(
            ErrorKind::InvalidSchema,
            span,
            format!(
                "prefix max node types {:?} does not match target schema {}",
                max_node_types, target.counts.node_type_limit
            ),
        ));
    }
    Ok(())
}

struct SchemaIdentity<'a> {
    schema: &'a str,
    base: &'a str,
}

impl<'a> SchemaIdentity<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let parts = text.split('_').collect::<Vec<_>>();
        if parts.len() != 4
            || parts[0] != "SCH"
            || !parts[1..]
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return None;
        }
        Some(Self {
            schema: parts[2],
            base: parts[3],
        })
    }
}

trait DocumentCursor {
    fn position(&self) -> usize;
    fn remaining(&self) -> usize;
    fn span_from(&self, start: usize) -> Span;
    fn read_u8(&mut self) -> Result<u8>;
    fn read_i16(&mut self) -> Result<Option<i16>>;
    fn read_u16(&mut self) -> Result<Option<u16>>;
    fn read_i32(&mut self) -> Result<Option<i32>>;
    fn read_index(&mut self) -> Result<Option<u32>>;
    fn read_final_index(&mut self) -> Result<Option<u32>>;
    fn read_f64(&mut self) -> Result<Option<f64>>;
    fn read_logical(&mut self) -> Result<bool>;
    fn read_char(&mut self) -> Result<u8>;
    fn read_chars(&mut self, count: usize) -> Result<Vec<u8>>;
    fn read_short_string(&mut self) -> Result<Vec<u8>>;
    fn read_vector(&mut self) -> Result<Option<[f64; 3]>>;
    fn read_interval(&mut self) -> Result<Option<[f64; 2]>>;
    fn read_box(&mut self) -> Result<Option<[f64; 6]>>;
}

impl DocumentCursor for TextCursor<'_> {
    fn position(&self) -> usize {
        TextCursor::position(self)
    }

    fn remaining(&self) -> usize {
        TextCursor::remaining(self)
    }

    fn span_from(&self, start: usize) -> Span {
        TextCursor::span_from(self, start)
    }

    fn read_u8(&mut self) -> Result<u8> {
        TextCursor::read_u8(self)
    }

    fn read_i16(&mut self) -> Result<Option<i16>> {
        TextCursor::read_i16(self)
    }

    fn read_u16(&mut self) -> Result<Option<u16>> {
        TextCursor::read_u16(self)
    }

    fn read_i32(&mut self) -> Result<Option<i32>> {
        TextCursor::read_i32(self)
    }

    fn read_index(&mut self) -> Result<Option<u32>> {
        TextCursor::read_index(self)
    }

    fn read_final_index(&mut self) -> Result<Option<u32>> {
        TextCursor::read_final_index(self)
    }

    fn read_f64(&mut self) -> Result<Option<f64>> {
        TextCursor::read_f64(self)
    }

    fn read_logical(&mut self) -> Result<bool> {
        TextCursor::read_logical(self)
    }

    fn read_char(&mut self) -> Result<u8> {
        TextCursor::read_char(self)
    }

    fn read_chars(&mut self, count: usize) -> Result<Vec<u8>> {
        TextCursor::read_chars(self, count)
    }

    fn read_short_string(&mut self) -> Result<Vec<u8>> {
        TextCursor::read_short_string(self)
    }

    fn read_vector(&mut self) -> Result<Option<[f64; 3]>> {
        TextCursor::read_vector(self)
    }

    fn read_interval(&mut self) -> Result<Option<[f64; 2]>> {
        TextCursor::read_interval(self)
    }

    fn read_box(&mut self) -> Result<Option<[f64; 6]>> {
        TextCursor::read_box(self)
    }
}

impl DocumentCursor for BinaryCursor<'_> {
    fn position(&self) -> usize {
        BinaryCursor::position(self)
    }

    fn remaining(&self) -> usize {
        BinaryCursor::remaining(self)
    }

    fn span_from(&self, start: usize) -> Span {
        BinaryCursor::span_from(self, start)
    }

    fn read_u8(&mut self) -> Result<u8> {
        BinaryCursor::read_u8(self)
    }

    fn read_i16(&mut self) -> Result<Option<i16>> {
        BinaryCursor::read_i16(self)
    }

    fn read_u16(&mut self) -> Result<Option<u16>> {
        BinaryCursor::read_u16(self)
    }

    fn read_i32(&mut self) -> Result<Option<i32>> {
        BinaryCursor::read_i32(self)
    }

    fn read_index(&mut self) -> Result<Option<u32>> {
        BinaryCursor::read_index(self)
    }

    fn read_final_index(&mut self) -> Result<Option<u32>> {
        BinaryCursor::read_final_index(self)
    }

    fn read_f64(&mut self) -> Result<Option<f64>> {
        BinaryCursor::read_f64(self)
    }

    fn read_logical(&mut self) -> Result<bool> {
        BinaryCursor::read_logical(self)
    }

    fn read_char(&mut self) -> Result<u8> {
        BinaryCursor::read_char(self)
    }

    fn read_chars(&mut self, count: usize) -> Result<Vec<u8>> {
        BinaryCursor::read_chars(self, count)
    }

    fn read_short_string(&mut self) -> Result<Vec<u8>> {
        BinaryCursor::read_short_string(self)
    }

    fn read_vector(&mut self) -> Result<Option<[f64; 3]>> {
        BinaryCursor::read_vector(self)
    }

    fn read_interval(&mut self) -> Result<Option<[f64; 2]>> {
        BinaryCursor::read_interval(self)
    }

    fn read_box(&mut self) -> Result<Option<[f64; 6]>> {
        BinaryCursor::read_box(self)
    }
}

fn read_first_node_schema<C: DocumentCursor>(
    cursor: &mut C,
    node_type: u16,
    base: &Schema,
    target: &NodeSpec,
) -> Result<(Vec<FieldSpec>, SchemaUpdateKind)> {
    let start = cursor.position();
    let flag = cursor.read_u8()?;
    let target_effective = effective_fields(target);
    let (embedded, update) = match base.node(node_type) {
        Some(base_node) if flag == u8::MAX => {
            (effective_fields(base_node), SchemaUpdateKind::BaseMatch)
        }
        Some(base_node) => {
            let mut edits = Vec::new();
            let mut instruction_count = 0usize;
            loop {
                instruction_count += 1;
                if instruction_count > 4096 {
                    return Err(XtError::new(
                        ErrorKind::InvalidSchemaEdit,
                        cursor.span_from(start),
                        "embedded schema edit has more than 4096 instructions",
                    ));
                }
                match cursor.read_char()? {
                    b'C' => edits.push(FieldEdit::Copy),
                    b'D' => edits.push(FieldEdit::Delete),
                    b'I' => edits.push(FieldEdit::Insert(read_embedded_field(cursor)?)),
                    b'A' => edits.push(FieldEdit::Append(read_embedded_field(cursor)?)),
                    b'Z' => break,
                    other => {
                        return Err(XtError::new(
                            ErrorKind::InvalidSchemaEdit,
                            cursor.span_from(start),
                            format!(
                                "unknown embedded schema instruction {:?}",
                                char::from(other)
                            ),
                        ));
                    }
                }
            }
            (
                apply_field_edits(&effective_fields(base_node), usize::from(flag), &edits)
                    .map_err(|error| {
                        XtError::new(
                            ErrorKind::InvalidSchemaEdit,
                            cursor.span_from(start),
                            format!("node {node_type}: {}", error.context),
                        )
                    })?,
                SchemaUpdateKind::Edited,
            )
        }
        None => {
            if flag == u8::MAX {
                return Err(XtError::new(
                    ErrorKind::InvalidSchemaEdit,
                    cursor.span_from(start),
                    "new node type cannot use base-match flag 255",
                ));
            }
            let _name = cursor.read_short_string()?;
            let _description = cursor.read_short_string()?;
            let mut fields = Vec::with_capacity(usize::from(flag));
            for _ in 0..flag {
                fields.push(read_embedded_field(cursor)?);
            }
            (fields, SchemaUpdateKind::NewNode)
        }
    };

    if !same_fields(&embedded, &target_effective) {
        let mismatch = first_field_mismatch(&embedded, &target_effective);
        return Err(XtError::new(
            ErrorKind::InvalidSchema,
            cursor.span_from(start),
            format!(
                "embedded effective layout for node {node_type} does not match target schema: {mismatch}"
            ),
        ));
    }
    Ok((embedded, update))
}

fn read_embedded_field<C: DocumentCursor>(cursor: &mut C) -> Result<FieldSpec> {
    let start = cursor.position();
    let name = utf8_string(
        cursor.read_short_string()?,
        cursor.span_from(start),
        "field name",
    )?;
    let pointer_class_value = cursor.read_i16()?.ok_or_else(|| {
        XtError::new(
            ErrorKind::InvalidSchemaEdit,
            cursor.span_from(start),
            "embedded pointer class cannot be unset",
        )
    })?;
    let pointer_class = u16::try_from(pointer_class_value).map_err(|_| {
        XtError::new(
            ErrorKind::InvalidSchemaEdit,
            cursor.span_from(start),
            "embedded pointer class must not be negative",
        )
    })?;
    let elements_value = cursor.read_index()?.ok_or_else(|| {
        XtError::new(
            ErrorKind::InvalidSchemaEdit,
            cursor.span_from(start),
            "embedded element count cannot be unset",
        )
    })?;
    let elements = ElementCount::from_schema_value(elements_value);
    let field_type = if pointer_class != 0 {
        FieldType::PointerIndex
    } else {
        let code = cursor.read_short_string()?;
        if code.len() != 1 {
            return Err(XtError::new(
                ErrorKind::InvalidSchemaEdit,
                cursor.span_from(start),
                "embedded field type must contain one character",
            ));
        }
        FieldType::from_code(code[0]).ok_or_else(|| {
            XtError::new(
                ErrorKind::InvalidSchemaEdit,
                cursor.span_from(start),
                format!("unknown embedded field type {:?}", char::from(code[0])),
            )
        })?
    };
    let transmitted = if elements == ElementCount::Variable {
        cursor.read_logical()?
    } else {
        true
    };
    Ok(FieldSpec {
        name,
        field_type,
        transmitted,
        pointer_class,
        elements,
        span: cursor.span_from(start),
    })
}

fn utf8_string(bytes: Vec<u8>, span: Span, context: &str) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| {
        XtError::new(
            ErrorKind::InvalidSchemaEdit,
            span,
            format!("embedded {context} is not UTF-8/ASCII"),
        )
    })
}

fn effective_fields(node: &NodeSpec) -> Vec<FieldSpec> {
    node.fields
        .iter()
        .filter(|field| field.transmitted || field.elements == ElementCount::Variable)
        .cloned()
        .collect()
}

fn same_fields(left: &[FieldSpec], right: &[FieldSpec]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same_field(left, right))
}

fn same_field(left: &FieldSpec, right: &FieldSpec) -> bool {
    left.name == right.name
        && left.field_type == right.field_type
        && left.transmitted == right.transmitted
        && left.pointer_class == right.pointer_class
        && left.elements == right.elements
}

fn first_field_mismatch(left: &[FieldSpec], right: &[FieldSpec]) -> String {
    if left.len() != right.len() {
        return format!("field count {} != {}", left.len(), right.len());
    }
    left.iter()
        .zip(right)
        .position(|(left, right)| !same_field(left, right))
        .map_or_else(
            || "unknown mismatch".to_owned(),
            |index| {
                format!(
                    "field {index}: embedded {:?}, target {:?}",
                    left[index].name, right[index].name
                )
            },
        )
}

fn read_node_fields<C: DocumentCursor>(
    cursor: &mut C,
    node: &NodeSpec,
    variable_length: Option<usize>,
) -> Result<Vec<FieldValue>> {
    let mut output = Vec::new();
    for field in node.fields.iter().filter(|field| field.transmitted) {
        let count = match field.elements {
            ElementCount::Scalar => 1,
            ElementCount::Variable => variable_length.ok_or_else(|| {
                node_error(
                    field.span,
                    format!(
                        "node {} field {} is variable but node has no VLA count",
                        node.node_type, field.name
                    ),
                )
            })?,
            ElementCount::Fixed(count) => usize::try_from(count).map_err(|_| {
                node_error(field.span, "fixed element count is outside usize range")
            })?,
        };
        let start = cursor.position();
        let values = if field.field_type == FieldType::Character {
            cursor
                .read_chars(count)?
                .into_iter()
                .map(ScalarValue::Character)
                .collect()
        } else {
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(read_scalar(cursor, field.field_type)?);
            }
            values
        };
        output.push(FieldValue {
            name: field.name.clone(),
            field_type: field.field_type,
            values,
            span: cursor.span_from(start),
        });
    }
    Ok(output)
}

fn read_scalar<C: DocumentCursor>(cursor: &mut C, field_type: FieldType) -> Result<ScalarValue> {
    Ok(match field_type {
        FieldType::UnsignedByte => ScalarValue::UnsignedByte(cursor.read_u8()?),
        FieldType::Character => ScalarValue::Character(cursor.read_char()?),
        FieldType::Logical => ScalarValue::Logical(cursor.read_logical()?),
        FieldType::ShortInteger => ScalarValue::ShortInteger(cursor.read_i16()?),
        FieldType::UnicodeCharacter => ScalarValue::UnicodeCharacter(cursor.read_u16()?),
        FieldType::Integer => ScalarValue::Integer(cursor.read_i32()?),
        FieldType::PointerIndex => ScalarValue::PointerIndex(cursor.read_index()?),
        FieldType::Double => ScalarValue::Double(cursor.read_f64()?),
        FieldType::Interval => ScalarValue::Interval(cursor.read_interval()?),
        FieldType::Vector => ScalarValue::Vector(cursor.read_vector()?),
        FieldType::Box => ScalarValue::Box(cursor.read_box()?),
        FieldType::IntersectionPoint => ScalarValue::IntersectionPoint(cursor.read_vector()?),
        FieldType::Tag | FieldType::Opaque => {
            return Err(XtError::new(
                ErrorKind::UnsupportedFieldType,
                cursor.span_from(cursor.position()),
                format!(
                    "unsupported {:?} field encountered in a transmitted node",
                    field_type
                ),
            ));
        }
    })
}

fn node_error(span: Span, context: impl Into<String>) -> XtError {
    XtError::new(ErrorKind::InvalidNode, span, context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SchemaFile;

    fn schema(key: &str) -> Schema {
        let input = format!(
            "preamble\n\
**PART1;FORMAT=text;GUISE=schema;KEY={key};\n\
**PART2;\n\
**PART3;\n\
**END_OF_HEADER***\n\
T \n\
1 \n\
: SCHEMA FILE test; \n\
3 2 1 0 \n\
1 NULLP; Null; 1 0 0 \n\
2 VALUE; Value; 1 1 0 \n\
value; d; 1 0 0 \n\
**************** end of schema {key} **************** \n"
        );
        SchemaFile::parse(input.as_bytes())
            .unwrap()
            .to_schema(None, None)
            .unwrap()
    }

    #[test]
    fn parses_one_node_with_base_match_and_terminator() {
        let base = schema("SCH_13006");
        let target = schema("SCH_14000");
        let schema_name = "SCH_1400000_14000_13006";
        let input = format!(
            "preamble\n\
**PART1;FORMAT=text;\n\
**PART2;SCH=SCH_1400000_14000;USFLD_SIZE=0;\n\
**PART3;\n\
**END_OF_HEADER***\n\
T1 X{} {}3 0 2 255 1 42 1 0 ",
            schema_name.len(),
            schema_name
        );
        let document = XtDocument::parse_text(input.as_bytes(), &base, &target).unwrap();
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.nodes[0].file_index, 1);
        assert_eq!(
            document.nodes[0].schema_update,
            Some(SchemaUpdateKind::BaseMatch)
        );
        assert_eq!(
            document.nodes[0].fields[0].values,
            [ScalarValue::Integer(Some(42))]
        );
        assert_eq!(document.terminator.index, 0);
    }

    #[test]
    fn parses_neutral_binary_node_graph() {
        let base = schema("SCH_13006");
        let target = schema("SCH_14000");
        let schema_name = b"SCH_1400000_14000_13006";
        let mut body = b"PS\0\0".to_vec();
        body.extend_from_slice(&1_i16.to_be_bytes());
        body.push(b'X');
        body.extend_from_slice(&(schema_name.len() as i32).to_be_bytes());
        body.extend_from_slice(schema_name);
        body.extend_from_slice(&3_i16.to_be_bytes());
        body.extend_from_slice(&0_i32.to_be_bytes());
        body.extend_from_slice(&2_i16.to_be_bytes());
        body.push(u8::MAX);
        body.extend_from_slice(&2_i16.to_be_bytes());
        body.extend_from_slice(&42_i32.to_be_bytes());
        body.extend_from_slice(&1_i16.to_be_bytes());
        body.extend_from_slice(&1_i16.to_be_bytes());

        let mut input = b"preamble\n\
**PART1;FORMAT=binary;\n\
**PART2;SCH=SCH_1400000_14000;USFLD_SIZE=0;\n\
**PART3;\n\
**END_OF_HEADER***\n"
            .to_vec();
        input.extend_from_slice(&body);

        let document = BinaryXtDocument::parse(&input, &base, &target).unwrap();
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.nodes[0].node_type, 2);
        assert_eq!(document.nodes[0].file_index, 1);
        assert_eq!(
            document.nodes[0].fields[0].values,
            vec![ScalarValue::Integer(Some(42))]
        );
        assert_eq!(document.terminator.index, 0);
    }
}
