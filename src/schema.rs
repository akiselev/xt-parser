use std::collections::BTreeSet;

use serde::Serialize;

use crate::{CommonHeader, ErrorKind, Result, Span, XtError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    UnsignedByte,
    Character,
    Logical,
    ShortInteger,
    UnicodeCharacter,
    Integer,
    PointerIndex,
    Double,
    Interval,
    Vector,
    Box,
    IntersectionPoint,
    /// Runtime opaque or callback value (`q`). This is retained by the schema
    /// importer but is not a documented XT part-stream field type.
    Opaque,
    /// Persistent tag (`t`). Retained for complete schema inventories.
    Tag,
}

impl FieldType {
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            b'u' => Self::UnsignedByte,
            b'c' => Self::Character,
            b'l' => Self::Logical,
            b'n' => Self::ShortInteger,
            b'w' => Self::UnicodeCharacter,
            b'd' => Self::Integer,
            b'p' => Self::PointerIndex,
            b'f' => Self::Double,
            b'i' => Self::Interval,
            b'v' => Self::Vector,
            b'b' => Self::Box,
            b'h' => Self::IntersectionPoint,
            b'q' => Self::Opaque,
            b't' => Self::Tag,
            _ => return None,
        })
    }

    pub const fn code(self) -> u8 {
        match self {
            Self::UnsignedByte => b'u',
            Self::Character => b'c',
            Self::Logical => b'l',
            Self::ShortInteger => b'n',
            Self::UnicodeCharacter => b'w',
            Self::Integer => b'd',
            Self::PointerIndex => b'p',
            Self::Double => b'f',
            Self::Interval => b'i',
            Self::Vector => b'v',
            Self::Box => b'b',
            Self::IntersectionPoint => b'h',
            Self::Opaque => b'q',
            Self::Tag => b't',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementCount {
    Scalar,
    Variable,
    Fixed(u32),
}

impl ElementCount {
    pub fn from_schema_value(value: u32) -> Self {
        match value {
            0 => Self::Scalar,
            1 => Self::Variable,
            fixed => Self::Fixed(fixed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldSpec {
    pub name: String,
    pub field_type: FieldType,
    pub transmitted: bool,
    pub pointer_class: u16,
    pub elements: ElementCount,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeSpec {
    pub node_type: u16,
    pub name: String,
    pub description: String,
    pub transmitted: bool,
    pub variable: bool,
    pub fields: Vec<FieldSpec>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SchemaCounts {
    /// One greater than the highest legal numeric node type.
    pub node_type_limit: u16,
    pub node_count: usize,
    pub field_count: usize,
    /// Fourth count from the vendor schema header. Its semantics are retained
    /// without interpretation.
    pub auxiliary_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaFile {
    pub header: CommonHeader,
    pub format_code: u32,
    pub creation_banner: String,
    pub counts: SchemaCounts,
    pub nodes: Vec<NodeSpec>,
    pub trailer: String,
}

/// Runtime schema definition used by the XT parser. Generated schema modules
/// construct this value directly, so deployed parsers do not need `.s_t` files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub key: String,
    pub creation_banner: String,
    pub counts: SchemaCounts,
    pub source_sha256: Option<String>,
    pub normalized_sha256: Option<String>,
    pub nodes: Vec<NodeSpec>,
}

impl Schema {
    pub fn node(&self, node_type: u16) -> Option<&NodeSpec> {
        self.nodes.iter().find(|node| node.node_type == node_type)
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl SchemaFile {
    /// Parse the vendor's line-oriented `.s_t` / `.sch_txt` schema format.
    pub fn parse(input: &[u8]) -> Result<Self> {
        let header = CommonHeader::parse(input)?;
        let lines = physical_lines(input, header.body_offset)?;
        if lines.len() < 5 {
            return Err(schema_error(
                Span::new(header.body_offset, input.len()),
                "schema body is too short",
            ));
        }
        if lines[0].text != "T" {
            return Err(schema_error(lines[0].span, "schema format flag must be T"));
        }
        let format_code = parse_scalar::<u32>(&lines[1], "schema format code")?;
        let creation_banner = lines[2].text.to_owned();
        let count_values = parse_columns::<u32>(&lines[3], 4, "schema counts")?;
        let counts = SchemaCounts {
            node_type_limit: u16::try_from(count_values[0])
                .map_err(|_| schema_error(lines[3].span, "node type limit is outside u16 range"))?,
            node_count: usize::try_from(count_values[1])
                .map_err(|_| schema_error(lines[3].span, "node count is outside usize range"))?,
            field_count: usize::try_from(count_values[2])
                .map_err(|_| schema_error(lines[3].span, "field count is outside usize range"))?,
            auxiliary_count: count_values[3],
        };

        let mut line_index = 4;
        let mut nodes = Vec::with_capacity(counts.node_count);
        let mut seen = BTreeSet::new();
        let mut actual_field_count = 0usize;
        while nodes.len() < counts.node_count {
            let line = lines.get(line_index).ok_or_else(|| {
                schema_error(
                    Span::new(header.body_offset, input.len()),
                    format!(
                        "schema ended after {} of {} declared nodes",
                        nodes.len(),
                        counts.node_count
                    ),
                )
            })?;
            let mut node = parse_node_line(line)?;
            if node.node_type >= counts.node_type_limit {
                return Err(schema_error(
                    line.span,
                    format!(
                        "node type {} is not below declared limit {}",
                        node.node_type, counts.node_type_limit
                    ),
                ));
            }
            if !seen.insert(node.node_type) {
                return Err(schema_error(
                    line.span,
                    format!("duplicate node type {}", node.node_type),
                ));
            }
            line_index += 1;

            let declared_fields = node.fields.capacity();
            for _ in 0..declared_fields {
                let field_line = lines.get(line_index).ok_or_else(|| {
                    schema_error(
                        line.span,
                        format!("node {} ended before all fields", node.node_type),
                    )
                })?;
                node.fields.push(parse_field_line(field_line)?);
                line_index += 1;
                actual_field_count += 1;
            }
            nodes.push(node);
        }

        if actual_field_count != counts.field_count {
            return Err(schema_error(
                lines[3].span,
                format!(
                    "parsed {actual_field_count} fields, header declares {}",
                    counts.field_count
                ),
            ));
        }
        let trailer_line = lines.get(line_index).ok_or_else(|| {
            schema_error(
                Span::new(header.body_offset, input.len()),
                "missing end-of-schema trailer",
            )
        })?;
        if !trailer_line
            .text
            .starts_with("**************** end of schema ")
        {
            return Err(schema_error(
                trailer_line.span,
                "invalid end-of-schema trailer",
            ));
        }
        line_index += 1;
        if let Some(extra) = lines[line_index..]
            .iter()
            .find(|line| !line.text.is_empty())
        {
            return Err(schema_error(
                extra.span,
                "unexpected content after end-of-schema trailer",
            ));
        }

        Ok(Self {
            header,
            format_code,
            creation_banner,
            counts,
            nodes,
            trailer: trailer_line.text.to_owned(),
        })
    }

    pub fn node(&self, node_type: u16) -> Option<&NodeSpec> {
        self.nodes.iter().find(|node| node.node_type == node_type)
    }

    pub fn schema_key(&self) -> Option<&[u8]> {
        self.header.value("KEY")
    }

    pub fn to_schema(
        &self,
        source_sha256: Option<String>,
        normalized_sha256: Option<String>,
    ) -> Result<Schema> {
        let key = self.schema_key().ok_or_else(|| {
            schema_error(self.header.header_span, "schema header has no KEY value")
        })?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|_| schema_error(self.header.header_span, "schema KEY is not UTF-8/ASCII"))?;
        Ok(Schema {
            key,
            creation_banner: self.creation_banner.clone(),
            counts: self.counts,
            source_sha256,
            normalized_sha256,
            nodes: self.nodes.clone(),
        })
    }
}

struct PhysicalLine<'a> {
    text: &'a str,
    span: Span,
}

fn physical_lines(input: &[u8], start: usize) -> Result<Vec<PhysicalLine<'_>>> {
    let mut output = Vec::new();
    let mut offset = start;
    while offset < input.len() {
        let relative_end = input[offset..]
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(input.len() - offset);
        let raw_end = offset + relative_end;
        let mut content_end = raw_end;
        while content_end > offset && matches!(input[content_end - 1], b'\r' | b' ') {
            content_end -= 1;
        }
        let text = std::str::from_utf8(&input[offset..content_end]).map_err(|_| {
            schema_error(
                Span::new(offset, raw_end),
                "schema physical line is not UTF-8/ASCII",
            )
        })?;
        output.push(PhysicalLine {
            text,
            span: Span::new(offset, raw_end),
        });
        offset = if raw_end < input.len() {
            raw_end + 1
        } else {
            raw_end
        };
    }
    Ok(output)
}

fn parse_node_line(line: &PhysicalLine<'_>) -> Result<NodeSpec> {
    let sections = line.text.split(';').collect::<Vec<_>>();
    if sections.len() != 3 {
        return Err(schema_error(
            line.span,
            "node declaration must contain two semicolons",
        ));
    }
    let (node_type, name) = sections[0].split_once(' ').ok_or_else(|| {
        schema_error(
            line.span,
            "node declaration must begin with '<type> <name>'",
        )
    })?;
    let node_type = node_type
        .parse::<u16>()
        .map_err(|_| schema_error(line.span, "invalid numeric node type"))?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(schema_error(line.span, "invalid schema node name"));
    }
    let tail = sections[2].split_ascii_whitespace().collect::<Vec<_>>();
    if tail.len() != 3 {
        return Err(schema_error(
            line.span,
            "node declaration tail must have transmit, field-count, and VLA values",
        ));
    }
    let transmitted = parse_flag(tail[0], line.span, "node transmit flag")?;
    let field_count = tail[1]
        .parse::<usize>()
        .map_err(|_| schema_error(line.span, "invalid node field count"))?;
    let variable = parse_flag(tail[2], line.span, "node VLA flag")?;

    Ok(NodeSpec {
        node_type,
        name: name.to_owned(),
        description: sections[1].trim().to_owned(),
        transmitted,
        variable,
        fields: Vec::with_capacity(field_count),
        span: line.span,
    })
}

fn parse_field_line(line: &PhysicalLine<'_>) -> Result<FieldSpec> {
    let sections = line.text.split(';').collect::<Vec<_>>();
    if sections.len() != 3 {
        return Err(schema_error(
            line.span,
            "field declaration must contain two semicolons",
        ));
    }
    let name = sections[0].trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(schema_error(line.span, "invalid schema field name"));
    }
    let type_text = sections[1].trim().as_bytes();
    if type_text.len() != 1 {
        return Err(schema_error(line.span, "field type must be one character"));
    }
    let field_type = FieldType::from_code(type_text[0]).ok_or_else(|| {
        schema_error(
            line.span,
            format!("unknown field type {:?}", char::from(type_text[0])),
        )
    })?;
    let tail = sections[2].split_ascii_whitespace().collect::<Vec<_>>();
    if tail.len() != 3 {
        return Err(schema_error(
            line.span,
            "field tail must have transmit, pointer-class, and element-count values",
        ));
    }
    let transmitted = parse_flag(tail[0], line.span, "field transmit flag")?;
    let pointer_class = tail[1]
        .parse::<u16>()
        .map_err(|_| schema_error(line.span, "invalid field pointer class"))?;
    let elements = tail[2]
        .parse::<u32>()
        .map(ElementCount::from_schema_value)
        .map_err(|_| schema_error(line.span, "invalid field element count"))?;
    if field_type != FieldType::PointerIndex && pointer_class != 0 {
        return Err(schema_error(
            line.span,
            "only pointer fields may have a nonzero pointer class",
        ));
    }

    Ok(FieldSpec {
        name: name.to_owned(),
        field_type,
        transmitted,
        pointer_class,
        elements,
        span: line.span,
    })
}

fn parse_flag(text: &str, span: Span, context: &str) -> Result<bool> {
    match text {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(schema_error(span, format!("{context} must be 0 or 1"))),
    }
}

fn parse_scalar<T>(line: &PhysicalLine<'_>, context: &str) -> Result<T>
where
    T: std::str::FromStr,
{
    line.text
        .parse::<T>()
        .map_err(|_| schema_error(line.span, format!("invalid {context}")))
}

fn parse_columns<T>(line: &PhysicalLine<'_>, count: usize, context: &str) -> Result<Vec<T>>
where
    T: std::str::FromStr,
{
    let values = line
        .text
        .split_ascii_whitespace()
        .map(str::parse::<T>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| schema_error(line.span, format!("invalid {context}")))?;
    if values.len() != count {
        return Err(schema_error(
            line.span,
            format!("{context} must contain exactly {count} values"),
        ));
    }
    Ok(values)
}

fn schema_error(span: Span, context: impl Into<String>) -> XtError {
    XtError::new(ErrorKind::InvalidSchema, span, context)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldEdit {
    Copy,
    Delete,
    Insert(FieldSpec),
    Append(FieldSpec),
}

/// Apply a documented V14+ `C/D/I/A/Z` field edit sequence to one base node.
///
/// Parsing the byte representation is intentionally separate: this pure
/// operation can be tested before a base-schema importer exists.
pub fn apply_field_edits(
    base: &[FieldSpec],
    expected_fields: usize,
    edits: &[FieldEdit],
) -> Result<Vec<FieldSpec>> {
    let mut base_position = 0;
    let mut output = Vec::with_capacity(expected_fields);
    let mut append_phase = false;

    for edit in edits {
        match edit {
            FieldEdit::Copy if !append_phase => {
                let field = base
                    .get(base_position)
                    .ok_or_else(|| edit_error("copy past base"))?;
                output.push(field.clone());
                base_position += 1;
            }
            FieldEdit::Delete if !append_phase => {
                if base.get(base_position).is_none() {
                    return Err(edit_error("delete past base"));
                }
                base_position += 1;
            }
            FieldEdit::Insert(field) if !append_phase => output.push(field.clone()),
            FieldEdit::Append(field) => {
                if base_position != base.len() {
                    return Err(edit_error("append before all base fields were consumed"));
                }
                append_phase = true;
                output.push(field.clone());
            }
            _ => return Err(edit_error("copy/delete/insert after append phase")),
        }
    }

    // The documented comparison loop stops when either list is exhausted.
    // Remaining current fields are explicit `A` records; remaining base fields
    // are therefore implicitly deleted.
    if output.len() != expected_fields {
        return Err(edit_error(&format!(
            "edit sequence produced {} fields, expected {expected_fields}",
            output.len()
        )));
    }
    Ok(output)
}

fn edit_error(context: &str) -> XtError {
    XtError::new(
        ErrorKind::InvalidSchemaEdit,
        Span::at(0),
        context.to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str) -> FieldSpec {
        FieldSpec {
            name: name.to_owned(),
            field_type: FieldType::Integer,
            transmitted: true,
            pointer_class: 0,
            elements: ElementCount::Scalar,
            span: Span::at(0),
        }
    }

    #[test]
    fn applies_copy_delete_insert_and_append() {
        let base = vec![field("a"), field("b"), field("c")];
        let edits = vec![
            FieldEdit::Copy,
            FieldEdit::Delete,
            FieldEdit::Insert(field("x")),
            FieldEdit::Copy,
            FieldEdit::Append(field("z")),
        ];
        let result = apply_field_edits(&base, 4, &edits).unwrap();
        assert_eq!(
            result
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "x", "c", "z"]
        );
    }

    #[test]
    fn rejects_edit_sequence_with_wrong_output_count() {
        let error = apply_field_edits(&[field("a")], 1, &[]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidSchemaEdit);
    }

    #[test]
    fn parses_schema_file_and_validates_counts() {
        let input = b"preamble\n\
**PART1;FORMAT=text;GUISE=schema;KEY=SCH_TEST;\n\
**PART2;\n\
**PART3;\n\
**END_OF_HEADER***\n\
T \n\
1 \n\
: SCHEMA FILE created for test; \n\
3 2 2 99 \n\
1 NULLP; Null; 1 0 0 \n\
2 VALUE; Value; 1 2 0 \n\
owner; p; 1 1 0 \n\
value; f; 1 0 0 \n\
**************** end of schema SCH_TEST **************** \n";
        let schema = SchemaFile::parse(input).unwrap();
        assert_eq!(schema.format_code, 1);
        assert_eq!(schema.counts.node_type_limit, 3);
        assert_eq!(schema.counts.node_count, 2);
        assert_eq!(schema.counts.field_count, 2);
        assert_eq!(schema.schema_key(), Some(b"SCH_TEST".as_slice()));
        assert_eq!(schema.node(2).unwrap().fields[1].name, "value");
    }
}
