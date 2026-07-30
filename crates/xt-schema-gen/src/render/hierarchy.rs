use std::fmt::Write as _;

use super::{RenderContext, write_generated_doc};

pub(crate) fn render(context: &RenderContext<'_>) -> String {
    let mut output = context.header();
    output.push_str("use xt_parser::TypedNode;\nuse super::nodes;\n\n");
    output.push_str(
        "/// An automatically inferred linked pointer-class family.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct FamilyInfo {\n\
         pub pointer_class: u16,\n\
         pub label: Option<&'static str>,\n\
         pub label_source_field: Option<&'static str>,\n\
         pub module_name: &'static str,\n\
         pub member_node_types: &'static [u16],\n\
         }\n\n\
         /// Families inferred from repeated `next`/`previous` links to the same\n\
         /// abstract pointer class. Labels come from the most frequent non-link\n\
         /// field that references the class.\n\
         pub const FAMILIES: &[FamilyInfo] = &[\n",
    );
    for family in &context.hierarchy.families {
        let label = option_str(family.label.as_deref());
        let label_source = option_str(family.label_source_field.as_deref());
        writeln!(
            output,
            "FamilyInfo {{ pointer_class: {}, label: {}, label_source_field: {}, module_name: {:?}, member_node_types: &{:?} }},",
            family.pointer_class,
            label,
            label_source,
            family.module_name,
            family.members,
        )
        .unwrap();
    }
    output.push_str("];\n\n");

    for family in &context.hierarchy.families {
        let label = family.label.as_deref().unwrap_or("unlabelled");
        write_generated_doc(
            &mut output,
            &format!(
                "Inferred family for pointer class {} (`{label}`)",
                family.pointer_class
            ),
        );
        writeln!(output, "pub trait {}: TypedNode {{}}", family.trait_name).unwrap();
        for node_type in &family.members {
            let Some(node) = context
                .schema
                .nodes
                .iter()
                .find(|node| node.node_type == *node_type && node.transmitted)
            else {
                continue;
            };
            let name = context.node_name(node.node_type);
            writeln!(output, "impl {} for nodes::{name} {{}}", family.trait_name).unwrap();
        }
        output.push('\n');
    }
    output
}

fn option_str(value: Option<&str>) -> String {
    value.map_or_else(|| "None".to_owned(), |value| format!("Some({value:?})"))
}
