use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;
use xt_schema::parse_text;

#[derive(Serialize)]
struct Report {
    root: PathBuf,
    files_discovered: usize,
    files_typed: usize,
    input_bytes: u64,
    raw_nodes: u64,
    typed_nodes: u64,
    node_types: BTreeMap<u16, usize>,
    errors: Vec<FileError>,
}

#[derive(Serialize)]
struct FileError {
    path: PathBuf,
    error: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xt-schema-inspect: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: xt-schema-inspect <model.x_t|corpus-directory>".to_owned())?;
    let mut paths = Vec::new();
    collect_paths(&root, &mut paths)?;
    paths.sort();
    paths.dedup();

    let mut report = Report {
        root,
        files_discovered: paths.len(),
        files_typed: 0,
        input_bytes: 0,
        raw_nodes: 0,
        typed_nodes: 0,
        node_types: BTreeMap::new(),
        errors: Vec::new(),
    };
    for path in paths {
        let input = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        report.input_bytes += input.len() as u64;
        match parse_text(&input) {
            Ok(document) => {
                report.files_typed += 1;
                report.raw_nodes += document.raw().nodes.len() as u64;
                // Conversion to typed nodes already happened for every record.
                report.typed_nodes += document.raw().nodes.len() as u64;
                for node in &document.raw().nodes {
                    *report.node_types.entry(node.node_type).or_default() += 1;
                }
            }
            Err(error) => report.errors.push(FileError {
                path,
                error: error.to_string(),
            }),
        }
    }
    let success = report.errors.is_empty();
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    if success {
        Ok(())
    } else {
        Err("one or more files failed typed conversion".to_owned())
    }
}

fn collect_paths(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        output.push(path.to_owned());
        return Ok(());
    }
    if !path.is_dir() {
        return Err(format!("{} is not a file or directory", path.display()));
    }
    for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let child = entry
            .map_err(|error| format!("{}: {error}", path.display()))?
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
