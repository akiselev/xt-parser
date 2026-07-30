use std::collections::{BTreeMap, BTreeSet};

use xt_parser::schema::{FieldType, Schema};

use crate::naming::{module_identifier, pascal_case};

const LINK_FIELDS: [&str; 2] = ["next", "previous"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyPlan {
    pub families: Vec<NodeFamily>,
    homes: BTreeMap<u16, u16>,
}

impl HierarchyPlan {
    pub fn family(&self, pointer_class: u16) -> Option<&NodeFamily> {
        self.families
            .iter()
            .find(|family| family.pointer_class == pointer_class)
    }

    pub fn home_family(&self, node_type: u16) -> Option<&NodeFamily> {
        self.homes
            .get(&node_type)
            .and_then(|pointer_class| self.family(*pointer_class))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeFamily {
    pub pointer_class: u16,
    pub label: Option<String>,
    pub label_source_field: Option<String>,
    pub module_name: String,
    pub trait_name: String,
    pub pointer_alias: String,
    pub members: Vec<u16>,
}

/// Infer schema node families without a hand-maintained node taxonomy.
///
/// A pointer class becomes a family when at least two node types contain a
/// `next` or `previous` field targeting that same abstract class. The family
/// label is the most frequently used non-link field name that points to the
/// class. This recovers the schema's own linked-class structure while keeping
/// the inference and its evidence inspectable.
pub fn infer_hierarchy(schema: &Schema) -> HierarchyPlan {
    let mut members = BTreeMap::<u16, BTreeSet<u16>>::new();
    let mut reference_names = BTreeMap::<u16, BTreeMap<String, usize>>::new();

    for node in &schema.nodes {
        for field in &node.fields {
            if field.field_type != FieldType::PointerIndex || field.pointer_class < 1000 {
                continue;
            }
            if LINK_FIELDS.contains(&field.name.as_str()) {
                members
                    .entry(field.pointer_class)
                    .or_default()
                    .insert(node.node_type);
            } else {
                *reference_names
                    .entry(field.pointer_class)
                    .or_default()
                    .entry(field.name.clone())
                    .or_default() += 1;
            }
        }
    }

    let mut families = members
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(pointer_class, members)| {
            let label_source_field = reference_names
                .get(&pointer_class)
                .and_then(most_frequent_name);
            let label = label_source_field
                .as_deref()
                .map(module_identifier)
                .filter(|label| !label.is_empty());
            let module_name = label.as_ref().map_or_else(
                || format!("class_{pointer_class}"),
                |label| format!("class_{pointer_class}_{label}"),
            );
            let type_stem = label.as_ref().map_or_else(
                || format!("PointerClass{pointer_class}"),
                |label| pascal_case(label),
            );
            NodeFamily {
                pointer_class,
                label,
                label_source_field,
                module_name,
                trait_name: format!("{type_stem}Family"),
                pointer_alias: format!("{type_stem}Class"),
                members: members.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    families.sort_by_key(|family| family.pointer_class);
    disambiguate_names(&mut families);

    let mut homes = BTreeMap::new();
    for family in &families {
        for node_type in &family.members {
            homes.entry(*node_type).or_insert(family.pointer_class);
        }
    }
    HierarchyPlan { families, homes }
}

fn most_frequent_name(counts: &BTreeMap<String, usize>) -> Option<String> {
    counts
        .iter()
        .max_by(|(left_name, left_count), (right_name, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_name.cmp(left_name))
        })
        .map(|(name, _)| name.clone())
}

fn disambiguate_names(families: &mut [NodeFamily]) {
    let mut trait_counts = BTreeMap::<String, usize>::new();
    let mut alias_counts = BTreeMap::<String, usize>::new();
    for family in families.iter() {
        *trait_counts.entry(family.trait_name.clone()).or_default() += 1;
        *alias_counts
            .entry(family.pointer_alias.clone())
            .or_default() += 1;
    }
    for family in families {
        if trait_counts[&family.trait_name] > 1 {
            family.trait_name = format!("{}{}", family.trait_name, family.pointer_class);
        }
        if alias_counts[&family.pointer_alias] > 1 {
            family.pointer_alias = format!("{}{}", family.pointer_alias, family.pointer_class);
        }
    }
}

#[cfg(test)]
mod tests {
    use xt_parser::{
        Span,
        schema::{ElementCount, FieldSpec, NodeSpec, SchemaCounts},
    };

    use super::*;

    fn pointer(name: &str, pointer_class: u16) -> FieldSpec {
        FieldSpec {
            name: name.to_owned(),
            field_type: FieldType::PointerIndex,
            transmitted: true,
            pointer_class,
            elements: ElementCount::Scalar,
            span: Span::at(0),
        }
    }

    fn node(node_type: u16, name: &str, fields: Vec<FieldSpec>) -> NodeSpec {
        NodeSpec {
            node_type,
            name: name.to_owned(),
            description: name.to_owned(),
            transmitted: true,
            variable: false,
            fields,
            span: Span::at(0),
        }
    }

    #[test]
    fn infers_family_membership_and_label_from_pointer_structure() {
        let schema = Schema {
            key: "SCH_TEST".to_owned(),
            creation_banner: "test".to_owned(),
            counts: SchemaCounts {
                node_type_limit: 4,
                node_count: 3,
                field_count: 5,
                auxiliary_count: 0,
            },
            source_sha256: None,
            normalized_sha256: None,
            nodes: vec![
                node(
                    1,
                    "PLANE",
                    vec![pointer("next", 1006), pointer("previous", 1006)],
                ),
                node(
                    2,
                    "SPHERE",
                    vec![pointer("next", 1006), pointer("previous", 1006)],
                ),
                node(3, "FACE", vec![pointer("surface", 1006)]),
            ],
        };

        let plan = infer_hierarchy(&schema);
        assert_eq!(plan.families.len(), 1);
        let family = &plan.families[0];
        assert_eq!(family.pointer_class, 1006);
        assert_eq!(family.label.as_deref(), Some("surface"));
        assert_eq!(family.label_source_field.as_deref(), Some("surface"));
        assert_eq!(family.module_name, "class_1006_surface");
        assert_eq!(family.trait_name, "SurfaceFamily");
        assert_eq!(family.pointer_alias, "SurfaceClass");
        assert_eq!(family.members, [1, 2]);
        assert_eq!(plan.home_family(1), Some(family));
        assert_eq!(plan.home_family(3), None);
    }

    #[test]
    fn ignores_single_link_without_a_repeated_class_family() {
        let schema = Schema {
            key: "SCH_TEST".to_owned(),
            creation_banner: "test".to_owned(),
            counts: SchemaCounts {
                node_type_limit: 2,
                node_count: 1,
                field_count: 1,
                auxiliary_count: 0,
            },
            source_sha256: None,
            normalized_sha256: None,
            nodes: vec![node(1, "ONLY", vec![pointer("next", 1099)])],
        };
        assert!(infer_hierarchy(&schema).families.is_empty());
    }
}
