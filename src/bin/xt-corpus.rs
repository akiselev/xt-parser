use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use xt_parser::{ScalarValue, SchemaUpdateKind, XtDocument, XtError, schema::SchemaFile};

#[derive(Debug, Serialize)]
struct CorpusReport {
    format_version: u32,
    root: PathBuf,
    corpus_sha256: String,
    base_schema_path: PathBuf,
    base_schema_sha256: String,
    target_schema_path: PathBuf,
    target_schema_sha256: String,
    files_discovered: usize,
    files_parsed: usize,
    input_bytes: u64,
    node_count: usize,
    field_value_count: usize,
    pointer_value_count: usize,
    unresolved_index_sets: usize,
    unresolved_pointer_count: usize,
    files_with_unresolved_indices: usize,
    unresolved_pointer_sources: BTreeMap<String, usize>,
    min_nodes_per_file: Option<usize>,
    max_nodes_per_file: Option<usize>,
    root_types: BTreeMap<String, usize>,
    node_types: BTreeMap<String, usize>,
    schema_updates: BTreeMap<String, usize>,
    header_diagnostics: BTreeMap<String, usize>,
    errors: Vec<FileError>,
}

#[derive(Debug, Serialize)]
struct FileError {
    path: PathBuf,
    sha256: String,
    error: XtError,
}

fn main() -> ExitCode {
    let arguments = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if arguments.len() != 3 {
        eprintln!("Usage: xt-corpus ROOT BASE_SCHEMA.s_t TARGET_SCHEMA.s_t");
        return ExitCode::from(2);
    }
    match run(&arguments[0], &arguments[1], &arguments[2]) {
        Ok(report) => {
            let success = report.errors.is_empty();
            if let Err(error) = serde_json::to_writer_pretty(std::io::stdout(), &report) {
                eprintln!("xt-corpus: serialize report: {error}");
                return ExitCode::from(2);
            }
            println!();
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(3)
            }
        }
        Err(error) => {
            eprintln!("xt-corpus: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(root: &Path, base_path: &Path, target_path: &Path) -> Result<CorpusReport, String> {
    let (base_bytes, base_schema_sha256) = read_hashed(base_path)?;
    let (target_bytes, target_schema_sha256) = read_hashed(target_path)?;
    let base = SchemaFile::parse(&base_bytes).map_err(|error| error.to_string())?;
    let target = SchemaFile::parse(&target_bytes).map_err(|error| error.to_string())?;
    let base = base
        .to_schema(Some(base_schema_sha256.clone()), None)
        .map_err(|error| error.to_string())?;
    let target = target
        .to_schema(Some(target_schema_sha256.clone()), None)
        .map_err(|error| error.to_string())?;

    let mut paths = Vec::new();
    collect_paths(root, &mut paths)?;
    paths.sort();
    paths.dedup();

    let mut report = CorpusReport {
        format_version: 1,
        root: root.to_path_buf(),
        corpus_sha256: String::new(),
        base_schema_path: base_path.to_path_buf(),
        base_schema_sha256,
        target_schema_path: target_path.to_path_buf(),
        target_schema_sha256,
        files_discovered: paths.len(),
        files_parsed: 0,
        input_bytes: 0,
        node_count: 0,
        field_value_count: 0,
        pointer_value_count: 0,
        unresolved_index_sets: 0,
        unresolved_pointer_count: 0,
        files_with_unresolved_indices: 0,
        unresolved_pointer_sources: BTreeMap::new(),
        min_nodes_per_file: None,
        max_nodes_per_file: None,
        root_types: BTreeMap::new(),
        node_types: BTreeMap::new(),
        schema_updates: BTreeMap::new(),
        header_diagnostics: BTreeMap::new(),
        errors: Vec::new(),
    };
    let mut corpus_hasher = Sha256::new();

    for path in paths {
        let input = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let digest = Sha256::digest(&input);
        let sha256 = format!("{digest:x}");
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        corpus_hasher.update(relative.as_bytes());
        corpus_hasher.update([0]);
        corpus_hasher.update(digest);
        report.input_bytes += input.len() as u64;

        match XtDocument::parse_text(&input, &base, &target) {
            Ok(document) => accumulate(&mut report, &document),
            Err(error) => report.errors.push(FileError {
                path,
                sha256,
                error,
            }),
        }
    }
    report.corpus_sha256 = format!("{:x}", corpus_hasher.finalize());
    Ok(report)
}

fn collect_paths(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        output.push(path.to_path_buf());
        return Ok(());
    }
    if !path.is_dir() {
        return Err(format!("{} is not a file or directory", path.display()));
    }
    for entry in fs::read_dir(path).map_err(|error| format!("read {}: {error}", path.display()))? {
        let child = entry
            .map_err(|error| format!("read entry under {}: {error}", path.display()))?
            .path();
        if child.is_dir() {
            collect_paths(&child, output)?;
        } else if is_xt_path(&child) {
            output.push(child);
        }
    }
    Ok(())
}

fn is_xt_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("x_t") || extension.eq_ignore_ascii_case("xt")
        })
}

fn accumulate(report: &mut CorpusReport, document: &XtDocument) {
    report.files_parsed += 1;
    let nodes = document.nodes.len();
    report.node_count += nodes;
    report.min_nodes_per_file = Some(
        report
            .min_nodes_per_file
            .map_or(nodes, |old| old.min(nodes)),
    );
    report.max_nodes_per_file = Some(
        report
            .max_nodes_per_file
            .map_or(nodes, |old| old.max(nodes)),
    );

    let root = &document.nodes[0];
    *report
        .root_types
        .entry(format!("{} {}", root.node_type, root.node_name))
        .or_insert(0) += 1;
    if !document.unresolved_pointer_indices.is_empty() {
        report.files_with_unresolved_indices += 1;
        report.unresolved_index_sets += document.unresolved_pointer_indices.len();
    }
    report.unresolved_pointer_count += document.unresolved_pointers.len();
    for pointer in &document.unresolved_pointers {
        *report
            .unresolved_pointer_sources
            .entry(format!(
                "{} {}",
                pointer.source_node_type, pointer.field_name
            ))
            .or_insert(0) += 1;
    }
    for diagnostic in &document.envelope.header.diagnostics {
        *report
            .header_diagnostics
            .entry(format!("{:?}", diagnostic.kind))
            .or_insert(0) += 1;
    }

    for node in &document.nodes {
        *report
            .node_types
            .entry(format!("{} {}", node.node_type, node.node_name))
            .or_insert(0) += 1;
        if let Some(update) = node.schema_update {
            let name = match update {
                SchemaUpdateKind::BaseMatch => "base_match",
                SchemaUpdateKind::Edited => "edited",
                SchemaUpdateKind::NewNode => "new_node",
            };
            *report.schema_updates.entry(name.to_owned()).or_insert(0) += 1;
        }
        for field in &node.fields {
            report.field_value_count += field.values.len();
            report.pointer_value_count += field
                .values
                .iter()
                .filter(|value| matches!(value, ScalarValue::PointerIndex(_)))
                .count();
        }
    }
}

fn read_hashed(path: &Path) -> Result<(Vec<u8>, String), String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok((bytes, sha256))
}
