use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::{Path, PathBuf},
};

use xt_parser::schema::{ElementCount, FieldSpec, FieldType, NodeSpec};

use crate::naming::field_identifiers;

use super::RenderContext;

pub(crate) fn render(
    context: &RenderContext<'_>,
    files: &mut BTreeMap<PathBuf, String>,
) -> Result<(), String> {
    let transmitted = context
        .schema
        .nodes
        .iter()
        .filter(|node| node.transmitted)
        .collect::<Vec<_>>();
    let mut family_nodes = BTreeMap::<u16, Vec<&NodeSpec>>::new();
    let mut standalone = Vec::new();
    for node in &transmitted {
        if let Some(family) = context.hierarchy.home_family(node.node_type) {
            family_nodes
                .entry(family.pointer_class)
                .or_default()
                .push(*node);
        } else {
            standalone.push(*node);
        }
    }

    files.insert(
        PathBuf::from("nodes/mod.rs"),
        render_module(context, &transmitted, &family_nodes),
    );
    files.insert(
        PathBuf::from("nodes/standalone.rs"),
        render_node_file(context, &standalone)?,
    );
    files.insert(
        PathBuf::from("nodes/families/mod.rs"),
        render_family_module(context, &family_nodes),
    );
    for (pointer_class, nodes) in family_nodes {
        let family = context
            .hierarchy
            .family(pointer_class)
            .expect("family map came from hierarchy");
        files.insert(
            Path::new("nodes")
                .join("families")
                .join(format!("{}.rs", family.module_name)),
            render_node_file(context, &nodes)?,
        );
    }
    Ok(())
}

fn render_module(
    context: &RenderContext<'_>,
    transmitted: &[&NodeSpec],
    family_nodes: &BTreeMap<u16, Vec<&NodeSpec>>,
) -> String {
    let mut output = context.header();
    output.push_str(
        "pub mod families;\n\
         mod standalone;\n\n\
         pub use families::*;\n\
         pub use standalone::*;\n\n\
         use serde::Serialize;\n\
         use xt_parser::{ErrorKind, NodeRecord, TypedNode, XtError};\n\n\
         /// Any transmitted node defined by this schema.\n\
         #[derive(Debug, Clone, PartialEq, Serialize)]\n\
         #[serde(tag = \"node\", content = \"value\")]\n\
         pub enum Node {\n",
    );
    for node in transmitted {
        let name = context.node_name(node.node_type);
        writeln!(output, "{name}({name}),").unwrap();
    }
    output.push_str("}\n\n");
    output.push_str(
        "impl TryFrom<&NodeRecord> for Node {\n\
         type Error = XtError;\n\
         fn try_from(record: &NodeRecord) -> Result<Self, Self::Error> {\n\
         match record.node_type {\n",
    );
    for node in transmitted {
        let name = context.node_name(node.node_type);
        writeln!(
            output,
            "{} => Ok(Self::{name}({name}::try_from_record(record)?)),",
            node.node_type
        )
        .unwrap();
    }
    output.push_str(
        "_ => Err(XtError::new(\n\
         ErrorKind::InvalidNode,\n\
         record.span,\n\
         format!(\"node type {} is not transmitted by this schema\", record.node_type),\n\
         )),\n\
         }\n\
         }\n\
         }\n",
    );

    if family_nodes.is_empty() {
        output.push_str("\n// This schema has no inferred linked pointer-class families.\n");
    }
    output
}

fn render_family_module(
    context: &RenderContext<'_>,
    family_nodes: &BTreeMap<u16, Vec<&NodeSpec>>,
) -> String {
    let mut output = context.header();
    for pointer_class in family_nodes.keys() {
        let family = context
            .hierarchy
            .family(*pointer_class)
            .expect("family map came from hierarchy");
        writeln!(output, "pub mod {};", family.module_name).unwrap();
    }
    if !family_nodes.is_empty() {
        output.push('\n');
    }
    for pointer_class in family_nodes.keys() {
        let family = context
            .hierarchy
            .family(*pointer_class)
            .expect("family map came from hierarchy");
        writeln!(output, "pub use {}::*;", family.module_name).unwrap();
    }
    output
}

fn render_node_file(context: &RenderContext<'_>, nodes: &[&NodeSpec]) -> Result<String, String> {
    let mut output = context.header();
    if nodes.is_empty() {
        output.push_str("// No transmitted nodes are assigned to this generated group.\n");
        return Ok(output);
    }
    writeln!(
        output,
        "use crate::schemas::{}::pointers;",
        context.module_name
    )
    .unwrap();
    output.push_str(
        "use serde::Serialize;\n\
         use xt_parser::{NodeRecord, TypedFields, TypedNode, XtError};\n\n",
    );
    for node in nodes {
        write_typed_node(&mut output, context, node)?;
    }
    Ok(output)
}

fn write_typed_node(
    output: &mut String,
    context: &RenderContext<'_>,
    node: &NodeSpec,
) -> Result<(), String> {
    let type_name = context.node_name(node.node_type);
    let fields = node
        .fields
        .iter()
        .filter(|field| field.transmitted)
        .collect::<Vec<_>>();
    let identifiers = field_identifiers(fields.iter().map(|field| field.name.as_str()));

    writeln!(
        output,
        "/// Node {} `{}`: {}",
        node.node_type, node.name, node.description
    )
    .unwrap();
    writeln!(
        output,
        "#[derive(Debug, Clone, PartialEq, Serialize)]\n\
         pub struct {type_name} {{\n\
         pub file_index: u32,"
    )
    .unwrap();
    if node.variable {
        output.push_str("pub variable_length: usize,\n");
    }
    for (field, identifier) in fields.iter().zip(&identifiers) {
        writeln!(
            output,
            "#[serde(rename = {:?})]\n\
             pub {identifier}: {},",
            field.name,
            rust_field_type(field, context)
        )
        .unwrap();
    }
    output.push_str("}\n\n");

    writeln!(
        output,
        "impl TypedNode for {type_name} {{\n\
         const NODE_TYPE: u16 = {};\n\
         const NODE_NAME: &'static str = {:?};\n\
         fn try_from_record(record: &NodeRecord) -> Result<Self, XtError> {{\n\
         #[allow(unused_mut)]\n\
         let mut fields = TypedFields::new(record, Self::NODE_TYPE, Self::NODE_NAME)?;",
        node.node_type, node.name
    )
    .unwrap();
    for (index, field) in fields.iter().enumerate() {
        let call = field_read_expression(field, context);
        writeln!(output, "let field_{index} = {call};").unwrap();
    }
    output.push_str("fields.finish()?;\nOk(Self {\nfile_index: record.file_index,\n");
    if node.variable {
        output.push_str(
            "variable_length: record.variable_length.ok_or_else(|| XtError::new(xt_parser::ErrorKind::InvalidNode, record.span, \"generated variable node has no length\"))?,\n",
        );
    }
    for (index, identifier) in identifiers.iter().enumerate() {
        writeln!(output, "{identifier}: field_{index},").unwrap();
    }
    output.push_str("})\n}\n}\n\n");
    Ok(())
}

fn field_read_expression(field: &FieldSpec, context: &RenderContext<'_>) -> String {
    if matches!(field.field_type, FieldType::Opaque | FieldType::Tag) {
        return match field.elements {
            ElementCount::Scalar => format!("fields.next_unsupported({:?})?", field.name),
            ElementCount::Variable => {
                format!("fields.next_unsupported_variable({:?})?", field.name)
            }
            ElementCount::Fixed(count) => format!(
                "fields.next_unsupported_fixed::<{}>({:?})?",
                count, field.name
            ),
        };
    }
    let base_type = rust_scalar_type(field, context);
    match field.elements {
        ElementCount::Scalar => {
            format!("fields.next_scalar::<{base_type}>({:?})?", field.name)
        }
        ElementCount::Variable => {
            format!("fields.next_variable::<{base_type}>({:?})?", field.name)
        }
        ElementCount::Fixed(count) => format!(
            "fields.next_fixed::<{base_type}, {}>({:?})?",
            count, field.name
        ),
    }
}

fn rust_field_type(field: &FieldSpec, context: &RenderContext<'_>) -> String {
    let scalar = rust_scalar_type(field, context);
    match field.elements {
        ElementCount::Scalar => scalar,
        ElementCount::Variable => format!("Vec<{scalar}>"),
        ElementCount::Fixed(count) => format!("[{scalar}; {count}]"),
    }
}

fn rust_scalar_type(field: &FieldSpec, context: &RenderContext<'_>) -> String {
    match field.field_type {
        FieldType::UnsignedByte => "xt_parser::XtByte".to_owned(),
        FieldType::Character => "xt_parser::XtChar".to_owned(),
        FieldType::Logical => "xt_parser::XtLogical".to_owned(),
        FieldType::ShortInteger => "xt_parser::XtShort".to_owned(),
        FieldType::UnicodeCharacter => "xt_parser::XtUnicode".to_owned(),
        FieldType::Integer => "xt_parser::XtInteger".to_owned(),
        FieldType::PointerIndex => {
            if field.pointer_class == 0 {
                "xt_parser::XtPointer<xt_parser::AnyNode>".to_owned()
            } else {
                format!(
                    "xt_parser::XtPointer<pointers::{}>",
                    context.pointer_alias(field.pointer_class)
                )
            }
        }
        FieldType::Double => "xt_parser::XtDouble".to_owned(),
        FieldType::Interval => "xt_parser::XtInterval".to_owned(),
        FieldType::Vector => "xt_parser::XtVector".to_owned(),
        FieldType::Box => "xt_parser::XtBox".to_owned(),
        FieldType::IntersectionPoint => "xt_parser::XtIntersectionPoint".to_owned(),
        FieldType::Opaque | FieldType::Tag => "xt_parser::XtUnsupported".to_owned(),
    }
}
