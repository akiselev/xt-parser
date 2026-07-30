use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use xt_parser::schema::SchemaFile;

#[derive(Debug, Serialize)]
struct Report {
    path: PathBuf,
    bytes: u64,
    sha256: String,
    #[serde(flatten)]
    result: Outcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Parsed {
        schema_key: Option<String>,
        format_code: u32,
        creation_banner: String,
        node_type_limit: u16,
        node_count: usize,
        field_count: usize,
        auxiliary_count: u32,
        transmitted_nodes: usize,
        effective_fields: usize,
        header_diagnostics: usize,
    },
    Error {
        message: String,
    },
}

fn main() -> ExitCode {
    let paths = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("Usage: xt-schema SCHEMA.s_t...");
        return ExitCode::from(2);
    }
    let reports = paths.iter().map(|path| inspect(path)).collect::<Vec<_>>();
    let failed = reports
        .iter()
        .any(|report| matches!(report.result, Outcome::Error { .. }));
    if let Err(error) = serde_json::to_writer_pretty(std::io::stdout(), &reports) {
        eprintln!("xt-schema: serialize report: {error}");
        return ExitCode::from(2);
    }
    println!();
    if failed {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn inspect(path: &Path) -> Report {
    let input = match fs::read(path) {
        Ok(input) => input,
        Err(error) => {
            return Report {
                path: path.to_path_buf(),
                bytes: 0,
                sha256: String::new(),
                result: Outcome::Error {
                    message: error.to_string(),
                },
            };
        }
    };
    let bytes = input.len() as u64;
    let sha256 = format!("{:x}", Sha256::digest(&input));
    let result = match SchemaFile::parse(&input) {
        Ok(schema) => Outcome::Parsed {
            schema_key: schema
                .schema_key()
                .map(|value| String::from_utf8_lossy(value).into_owned()),
            format_code: schema.format_code,
            creation_banner: schema.creation_banner,
            node_type_limit: schema.counts.node_type_limit,
            node_count: schema.counts.node_count,
            field_count: schema.counts.field_count,
            auxiliary_count: schema.counts.auxiliary_count,
            transmitted_nodes: schema.nodes.iter().filter(|node| node.transmitted).count(),
            effective_fields: schema
                .nodes
                .iter()
                .flat_map(|node| &node.fields)
                .filter(|field| {
                    field.transmitted
                        || matches!(field.elements, xt_parser::schema::ElementCount::Variable)
                })
                .count(),
            header_diagnostics: schema.header.diagnostics.len(),
        },
        Err(error) => Outcome::Error {
            message: error.to_string(),
        },
    };
    Report {
        path: path.to_path_buf(),
        bytes,
        sha256,
        result,
    }
}
