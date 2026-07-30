use std::fmt::Write as _;

use super::{RenderContext, write_generated_doc};

pub(crate) fn render(context: &RenderContext<'_>) -> String {
    let mut output = context.header();
    output.push_str("use xt_parser::{AbstractClass, ExactNode};\n\n");
    output.push_str(
        "/// Exact-node pointer targets declared by this schema.\n\
         pub mod exact {\n\
         use super::ExactNode;\n",
    );
    for node in &context.schema.nodes {
        let node_name = context.node_name(node.node_type);
        write_generated_doc(
            &mut output,
            &format!(
                "Pointer target for node {} `{}` ({})",
                node.node_type, node.name, node.description
            ),
        );
        writeln!(
            output,
            "pub type {node_name}Node = ExactNode<{}>;",
            node.node_type
        )
        .unwrap();
    }
    output.push_str("}\n\n");

    output.push_str(
        "/// Abstract pointer-class targets declared by schema fields.\n\
         pub mod classes {\n\
         use super::AbstractClass;\n",
    );
    for pointer_class in &context.abstract_classes {
        let alias = context.pointer_alias(*pointer_class);
        if let Some(family) = context.hierarchy.family(*pointer_class) {
            let label = family.label.as_deref().unwrap_or("unlabelled");
            write_generated_doc(
                &mut output,
                &format!(
                    "Pointer class {pointer_class}; inferred family label `{label}` from schema links"
                ),
            );
        } else {
            write_generated_doc(
                &mut output,
                &format!("Pointer class {pointer_class}; no node family was inferred"),
            );
        }
        writeln!(output, "pub type {alias} = AbstractClass<{pointer_class}>;").unwrap();
    }
    output.push_str("}\n\npub use classes::*;\npub use exact::*;\n");
    output
}
