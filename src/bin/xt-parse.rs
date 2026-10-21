use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode};

use serde::Serialize;
use sha2::{Digest, Sha256};
use xt_parser::{ScalarValue, SchemaUpdateKind, XtDocument, XtError, schema::SchemaFile};

#[derive(Debug, Serialize)]
struct Report {
    format_version: u32,
    xt_path: PathBuf,
    xt_sha256: String,
    base_schema_path: PathBuf,
    base_schema_sha256: String,
    target_schema_path: PathBuf,
    target_schema_sha256: String,
    #[serde(flatten)]
    result: Outcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Parsed {
        schema_name: String,
        root: NodeSummary,
        node_count: usize,
        max_file_index: u32,
        node_types: BTreeMap<String, usize>,
        schema_updates: BTreeMap<String, usize>,
        field_value_count: usize,
        pointer_value_count: usize,
        null_pointer_count: usize,
        unset_pointer_count: usize,
        unresolved_pointer_indices: Vec<u32>,
        unresolved_pointers: Vec<xt_parser::UnresolvedPointer>,
        header_diagnostic_count: usize,
        logical_body_bytes: usize,
        terminator_index: u32,
    },
    ParseError {
        error: XtError,
    },
    ToolError {
        message: String,
    },
}

#[derive(Debug, Serialize)]
struct NodeSummary {
    node_type: u16,
    node_name: String,
    file_index: u32,
}

fn main() -> ExitCode {
    let arguments = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if arguments.len() != 3 {
        eprintln!("Usage: xt-parse MODEL.x_t BASE_SCHEMA.s_t TARGET_SCHEMA.s_t");
        return ExitCode::from(2);
    }
    let report = inspect(&arguments[0], &arguments[1], &arguments[2]);
    let success = matches!(report.result, Outcome::Parsed { .. });
    if let Err(error) = serde_json::to_writer_pretty(std::io::stdout(), &report) {
        eprintln!("xt-parse: serialize report: {error}");
        return ExitCode::from(2);
    }
    println!();
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn inspect(xt_path: &PathBuf, base_path: &PathBuf, target_path: &PathBuf) -> Report {
    let xt = read_hashed(xt_path);
    let base = read_hashed(base_path);
    let target = read_hashed(target_path);
    let (xt_bytes, xt_sha256, base_bytes, base_sha256, target_bytes, target_sha256) =
        match (xt, base, target) {
            (Ok((xt, xh)), Ok((base, bh)), Ok((target, th))) => (xt, xh, base, bh, target, th),
            (xt, base, target) => {
                let message = [xt.err(), base.err(), target.err()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ");
                return Report {
                    format_version: 1,
                    xt_path: xt_path.clone(),
                    xt_sha256: String::new(),
                    base_schema_path: base_path.clone(),
                    base_schema_sha256: String::new(),
                    target_schema_path: target_path.clone(),
                    target_schema_sha256: String::new(),
                    result: Outcome::ToolError { message },
                };
            }
        };

    let result = match (
        SchemaFile::parse(&base_bytes),
        SchemaFile::parse(&target_bytes),
    ) {
        (Ok(base), Ok(target)) => {
            let base = base.to_schema(Some(base_sha256.clone()), None);
            let target = target.to_schema(Some(target_sha256.clone()), None);
            match (base, target) {
                (Ok(base), Ok(target)) => match XtDocument::parse_text(&xt_bytes, &base, &target) {
                    Ok(document) => summarize(&document),
                    Err(error) => Outcome::ParseError { error },
                },
                (base, target) => Outcome::ToolError {
                    message: [base.err(), target.err()]
                        .into_iter()
                        .flatten()
                        .map(|error| error.to_string())
                        .collect::<Vec<_>>()
                        .join("; "),
                },
            }
        }
        (base, target) => Outcome::ToolError {
            message: [base.err(), target.err()]
                .into_iter()
                .flatten()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        },
    };

    Report {
        format_version: 1,
        xt_path: xt_path.clone(),
        xt_sha256,
        base_schema_path: base_path.clone(),
        base_schema_sha256: base_sha256,
        target_schema_path: target_path.clone(),
        target_schema_sha256: target_sha256,
        result,
    }
}

fn summarize(document: &XtDocument) -> Outcome {
    let mut node_types = BTreeMap::new();
    let mut schema_updates = BTreeMap::new();
    let mut field_value_count = 0usize;
    let mut pointer_value_count = 0usize;
    let mut null_pointer_count = 0usize;
    let mut unset_pointer_count = 0usize;

    for node in &document.nodes {
        *node_types
            .entry(format!("{} {}", node.node_type, node.node_name))
            .or_insert(0) += 1;
        if let Some(update) = node.schema_update {
            let key = match update {
                SchemaUpdateKind::BaseMatch => "base_match",
                SchemaUpdateKind::Edited => "edited",
                SchemaUpdateKind::NewNode => "new_node",
            };
            *schema_updates.entry(key.to_owned()).or_insert(0) += 1;
        }
        for field in &node.fields {
            field_value_count += field.values.len();
            for value in &field.values {
                match value {
                    ScalarValue::PointerIndex(Some(0)) => {
                        pointer_value_count += 1;
                        null_pointer_count += 1;
                    }
                    ScalarValue::PointerIndex(Some(_)) => pointer_value_count += 1,
                    ScalarValue::PointerIndex(None) => {
                        pointer_value_count += 1;
                        unset_pointer_count += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    let root = &document.nodes[0];
    Outcome::Parsed {
        schema_name: String::from_utf8_lossy(&document.envelope.prefix.schema_name).into_owned(),
        root: NodeSummary {
            node_type: root.node_type,
            node_name: root.node_name.clone(),
            file_index: root.file_index,
        },
        node_count: document.nodes.len(),
        max_file_index: document
            .nodes
            .iter()
            .map(|node| node.file_index)
            .max()
            .unwrap_or(0),
        node_types,
        schema_updates,
        field_value_count,
        pointer_value_count,
        null_pointer_count,
        unset_pointer_count,
        unresolved_pointer_indices: document
            .unresolved_pointer_indices
            .iter()
            .copied()
            .collect(),
        unresolved_pointers: document.unresolved_pointers.clone(),
        header_diagnostic_count: document
            .envelope
            .header
            .as_ref()
            .map_or(0, |header| header.diagnostics.len()),
        logical_body_bytes: document.envelope.stream.logical_bytes().len(),
        terminator_index: document.terminator.index,
    }
}

fn read_hashed(path: &PathBuf) -> Result<(Vec<u8>, String), String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok((bytes, sha256))
}
