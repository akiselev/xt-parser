//! Generated Parasolid schema metadata and typed XT node APIs.
//!
//! Raw `.s_t` files are code-generation inputs only. Generated modules isolate
//! source identity, parser specifications, pointer targets, inferred node
//! families, and strongly typed transmitted records.

pub mod schemas;
pub mod view;

use xt_parser::{BinaryXtDocument, MachineEncoding, Result, XtDocument, schema::SchemaCounts};

/// Provenance and inventory for one generated schema module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMetadata {
    pub source_name: &'static str,
    pub source_sha256: &'static str,
    pub normalized_sha256: &'static str,
    pub schema_key: &'static str,
    pub creation_banner: &'static str,
    pub counts: SchemaCounts,
}

/// A text XT document with every raw record converted to a generated node enum.
#[derive(Debug, Clone)]
pub struct TypedTextDocument<N> {
    pub raw: XtDocument,
    pub nodes: Vec<N>,
}

/// A binary XT document with every raw record converted to a generated node enum.
#[derive(Debug, Clone)]
pub struct TypedBinaryDocument<N> {
    pub raw: BinaryXtDocument,
    pub nodes: Vec<N>,
}

/// Parse Onshape/ABCD schema-30100 text using its V13 schema-13006 base.
pub fn parse_text_30100(input: &[u8]) -> Result<TypedTextDocument<schemas::sch_30100::Node>> {
    let raw = XtDocument::parse_text(
        input,
        &schemas::sch_13006::SCHEMA,
        &schemas::sch_30100::SCHEMA,
    )?;
    let nodes = raw
        .nodes
        .iter()
        .map(schemas::sch_30100::Node::try_from)
        .collect::<Result<Vec<_>>>()?;
    Ok(TypedTextDocument { raw, nodes })
}

/// Parse schema-37102 text and convert every record to its generated node type.
pub fn parse_text_37102(input: &[u8]) -> Result<TypedTextDocument<schemas::sch_37102::Node>> {
    let raw = XtDocument::parse_text(
        input,
        &schemas::sch_13006::SCHEMA,
        &schemas::sch_37102::SCHEMA,
    )?;
    let nodes = raw
        .nodes
        .iter()
        .map(schemas::sch_37102::Node::try_from)
        .collect::<Result<Vec<_>>>()?;
    Ok(TypedTextDocument { raw, nodes })
}

/// Parse neutral or typed schema-37102 binary XT.
pub fn parse_binary_37102(input: &[u8]) -> Result<TypedBinaryDocument<schemas::sch_37102::Node>> {
    parse_binary_37102_with_machine(input, None)
}

/// Parse schema-37102 binary XT, supplying the machine representation required
/// by a bare binary stream. Neutral and typed streams ignore `bare_machine`.
pub fn parse_binary_37102_with_machine(
    input: &[u8],
    bare_machine: Option<MachineEncoding>,
) -> Result<TypedBinaryDocument<schemas::sch_37102::Node>> {
    let raw = BinaryXtDocument::parse_with_bare_machine(
        input,
        &schemas::sch_13006::SCHEMA,
        &schemas::sch_37102::SCHEMA,
        bare_machine,
    )?;
    let nodes = raw
        .nodes
        .iter()
        .map(schemas::sch_37102::Node::try_from)
        .collect::<Result<Vec<_>>>()?;
    Ok(TypedBinaryDocument { raw, nodes })
}

#[cfg(test)]
mod tests {
    use xt_parser::TypedNode;

    use super::*;

    #[test]
    fn embeds_exact_schema_identities_and_counts() {
        let cases = [
            (
                schemas::sch_13006::metadata::METADATA,
                "SCH_13006",
                142,
                1_125,
                "0dd291ea706fc306f16a78140e05e595e75c85ab63e4077e68941b127fcfdcc3",
            ),
            (
                schemas::sch_30100::metadata::METADATA,
                "SCH_30100",
                184,
                1_402,
                "18838cb90466615a57c841896d3842044e97e85f2b5e33b62c41222f4e37ece5",
            ),
            (
                schemas::sch_37102::metadata::METADATA,
                "SCH_37102",
                206,
                1_556,
                "c2d06b56144524347834699b33d9dbfe7e6a1b8d3cd125b1a4d68593464f8e5f",
            ),
        ];
        for (metadata, key, nodes, fields, normalized_hash) in cases {
            assert_eq!(metadata.schema_key, key);
            assert_eq!(metadata.counts.node_count, nodes);
            assert_eq!(metadata.counts.field_count, fields);
            assert_eq!(metadata.normalized_sha256, normalized_hash);
        }
    }

    #[test]
    fn generated_nodes_and_pointer_targets_retain_schema_identity() {
        use schemas::sch_30100::{nodes::Body, pointers::BodyNode};

        assert_eq!(Body::NODE_TYPE, 12);
        assert_eq!(Body::NODE_NAME, "BODY");
        assert_eq!(BodyNode::NODE_TYPE, 12);
    }

    #[test]
    fn hierarchy_is_inferred_and_usable_as_rust_traits() {
        use schemas::sch_37102::{
            hierarchy::{FAMILIES, SurfaceFamily},
            nodes::Plane,
        };

        fn require_surface_family<T: SurfaceFamily>() {}
        require_surface_family::<Plane>();

        let surface = FAMILIES
            .iter()
            .find(|family| family.pointer_class == 1006)
            .unwrap();
        assert_eq!(surface.label, Some("surface"));
        assert_eq!(surface.label_source_field, Some("surface"));
        assert!(surface.member_node_types.contains(&50));
    }
}
