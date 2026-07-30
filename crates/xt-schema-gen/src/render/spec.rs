use std::fmt::Write as _;

use xt_parser::schema::{ElementCount, FieldType};

use super::RenderContext;

pub(crate) fn render(context: &RenderContext<'_>) -> String {
    let mut output = context.header();
    output.push_str(
        "use std::sync::LazyLock;\n\
         use xt_parser::Span;\n\
         use xt_parser::schema::{ElementCount, FieldSpec, FieldType, NodeSpec, Schema};\n\
         use super::metadata::{COUNTS, CREATION_BANNER, NORMALIZED_SHA256, SCHEMA_KEY, SOURCE_SHA256};\n\n",
    );
    output.push_str(
        "pub static SCHEMA: LazyLock<Schema> = LazyLock::new(|| Schema {\n\
         key: SCHEMA_KEY.to_owned(),\n\
         creation_banner: CREATION_BANNER.to_owned(),\n\
         counts: COUNTS,\n\
         source_sha256: Some(SOURCE_SHA256.to_owned()),\n\
         normalized_sha256: Some(NORMALIZED_SHA256.to_owned()),\n\
         nodes: vec![\n",
    );
    for node in &context.schema.nodes {
        writeln!(
            output,
            "NodeSpec {{ node_type: {}, name: {:?}.to_owned(), description: {:?}.to_owned(), transmitted: {}, variable: {}, span: Span::new({}, {}), fields: vec![",
            node.node_type,
            node.name,
            node.description,
            node.transmitted,
            node.variable,
            node.span.start,
            node.span.end,
        )
        .unwrap();
        for field in &node.fields {
            writeln!(
                output,
                "FieldSpec {{ name: {:?}.to_owned(), field_type: FieldType::{}, transmitted: {}, pointer_class: {}, elements: {}, span: Span::new({}, {}) }},",
                field.name,
                field_type_variant(field.field_type),
                field.transmitted,
                field.pointer_class,
                element_expression(field.elements),
                field.span.start,
                field.span.end,
            )
            .unwrap();
        }
        output.push_str("]},\n");
    }
    output.push_str("],\n});\n");
    output
}

fn field_type_variant(field_type: FieldType) -> &'static str {
    match field_type {
        FieldType::UnsignedByte => "UnsignedByte",
        FieldType::Character => "Character",
        FieldType::Logical => "Logical",
        FieldType::ShortInteger => "ShortInteger",
        FieldType::UnicodeCharacter => "UnicodeCharacter",
        FieldType::Integer => "Integer",
        FieldType::PointerIndex => "PointerIndex",
        FieldType::Double => "Double",
        FieldType::Interval => "Interval",
        FieldType::Vector => "Vector",
        FieldType::Box => "Box",
        FieldType::IntersectionPoint => "IntersectionPoint",
        FieldType::Opaque => "Opaque",
        FieldType::Tag => "Tag",
    }
}

fn element_expression(elements: ElementCount) -> String {
    match elements {
        ElementCount::Scalar => "ElementCount::Scalar".to_owned(),
        ElementCount::Variable => "ElementCount::Variable".to_owned(),
        ElementCount::Fixed(count) => format!("ElementCount::Fixed({count})"),
    }
}
