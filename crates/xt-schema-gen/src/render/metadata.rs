use std::fmt::Write as _;

use super::RenderContext;

pub(crate) fn render(context: &RenderContext<'_>) -> String {
    let mut output = context.header();
    output.push_str("use crate::SchemaMetadata;\nuse xt_parser::schema::SchemaCounts;\n\n");
    writeln!(
        output,
        "pub const SOURCE_NAME: &str = {:?};\n\
         pub const SOURCE_SHA256: &str = {:?};\n\
         pub const NORMALIZED_SHA256: &str = {:?};\n\
         pub const SCHEMA_KEY: &str = {:?};\n\
         pub const CREATION_BANNER: &str = {:?};\n\
         pub const COUNTS: SchemaCounts = SchemaCounts {{\n\
         node_type_limit: {},\n\
         node_count: {},\n\
         field_count: {},\n\
         auxiliary_count: {},\n\
         }};\n\
         pub const METADATA: SchemaMetadata = SchemaMetadata {{\n\
         source_name: SOURCE_NAME,\n\
         source_sha256: SOURCE_SHA256,\n\
         normalized_sha256: NORMALIZED_SHA256,\n\
         schema_key: SCHEMA_KEY,\n\
         creation_banner: CREATION_BANNER,\n\
         counts: COUNTS,\n\
         }};",
        context.identity.source_name,
        context.identity.source_sha256,
        context.identity.normalized_sha256,
        context.schema.key,
        context.schema.creation_banner,
        context.schema.counts.node_type_limit,
        context.schema.counts.node_count,
        context.schema.counts.field_count,
        context.schema.counts.auxiliary_count,
    )
    .unwrap();
    output
}
