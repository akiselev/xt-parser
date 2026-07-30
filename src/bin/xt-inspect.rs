use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use xt_parser::{CommonHeader, TextEnvelope, XtError};

#[derive(Debug, Serialize)]
struct FileReport {
    path: PathBuf,
    bytes: u64,
    sha256: String,
    #[serde(flatten)]
    result: Inspection,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Inspection {
    Parsed {
        header: HeaderReport,
        text: Option<TextReport>,
    },
    ParseError {
        error: XtError,
    },
    IoError {
        message: String,
    },
}

#[derive(Debug, Serialize)]
struct HeaderReport {
    body_offset: usize,
    values: BTreeMap<String, Vec<String>>,
    diagnostics: Vec<xt_parser::HeaderDiagnostic>,
}

#[derive(Debug, Serialize)]
struct TextReport {
    logical_body_bytes: usize,
    modeler_version: String,
    schema_name: String,
    max_node_types: Option<u16>,
    user_field_size: u8,
    node_start_logical: usize,
    undecoded_node_bytes: usize,
}

fn main() -> ExitCode {
    match run() {
        Ok(had_errors) => {
            if had_errors {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(message) => {
            eprintln!("xt-inspect: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> std::result::Result<bool, String> {
    let mut pretty = false;
    let mut roots = Vec::new();
    for argument in env::args_os().skip(1) {
        if argument == "--pretty" {
            pretty = true;
        } else if argument == "--help" || argument == "-h" {
            print_help();
            return Ok(false);
        } else {
            roots.push(PathBuf::from(argument));
        }
    }
    if roots.is_empty() {
        return Err("provide one or more XT files or directories; see --help".to_owned());
    }

    let mut paths = Vec::new();
    let mut discovery_errors = Vec::new();
    for root in roots {
        collect_paths(&root, &mut paths, &mut discovery_errors);
    }
    paths.sort();
    paths.dedup();

    let mut reports = Vec::with_capacity(paths.len() + discovery_errors.len());
    for (path, message) in discovery_errors {
        reports.push(FileReport {
            path,
            bytes: 0,
            sha256: String::new(),
            result: Inspection::IoError { message },
        });
    }
    for path in paths {
        reports.push(inspect(path));
    }
    reports.sort_by(|left, right| left.path.cmp(&right.path));
    let had_errors = reports
        .iter()
        .any(|report| !matches!(report.result, Inspection::Parsed { .. }));

    let stdout = io::stdout();
    let mut lock = stdout.lock();
    if pretty {
        serde_json::to_writer_pretty(&mut lock, &reports)
    } else {
        serde_json::to_writer(&mut lock, &reports)
    }
    .map_err(|error| format!("serialize report: {error}"))?;
    writeln!(lock).map_err(|error| format!("write report: {error}"))?;
    Ok(had_errors)
}

fn print_help() {
    println!(
        "Usage: xt-inspect [--pretty] PATH...\n\
         \n\
         Recursively inspect .x_t and .xt files and write one deterministic JSON array.\n\
         Exit status 2 means at least one file could not be read or strictly parsed."
    );
}

fn collect_paths(path: &Path, output: &mut Vec<PathBuf>, errors: &mut Vec<(PathBuf, String)>) {
    if path.is_file() {
        output.push(path.to_path_buf());
        return;
    }
    if !path.is_dir() {
        errors.push((
            path.to_path_buf(),
            "path is not a file or directory".to_owned(),
        ));
        return;
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push((path.to_path_buf(), error.to_string()));
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => {
                let child = entry.path();
                if child.is_dir() {
                    collect_paths(&child, output, errors);
                } else if is_xt_path(&child) {
                    output.push(child);
                }
            }
            Err(error) => errors.push((path.to_path_buf(), error.to_string())),
        }
    }
}

fn is_xt_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("x_t") || extension.eq_ignore_ascii_case("xt")
        })
}

fn inspect(path: PathBuf) -> FileReport {
    let input = match fs::read(&path) {
        Ok(input) => input,
        Err(error) => {
            return FileReport {
                path,
                bytes: 0,
                sha256: String::new(),
                result: Inspection::IoError {
                    message: error.to_string(),
                },
            };
        }
    };
    let digest = Sha256::digest(&input);
    let sha256 = format!("{digest:x}");
    let bytes = input.len() as u64;

    let result = match CommonHeader::parse(&input) {
        Err(error) => Inspection::ParseError { error },
        Ok(header) => {
            let header_report = header_report(&header);
            let is_text = header
                .value("FORMAT")
                .is_some_and(|value| value.eq_ignore_ascii_case(b"text"));
            if is_text {
                match TextEnvelope::parse(&input) {
                    Ok(envelope) => Inspection::Parsed {
                        header: header_report,
                        text: Some(TextReport {
                            logical_body_bytes: envelope.stream.logical_bytes().len(),
                            modeler_version: display_bytes(&envelope.prefix.modeler_version),
                            schema_name: display_bytes(&envelope.prefix.schema_name),
                            max_node_types: envelope.prefix.max_node_types,
                            user_field_size: envelope.prefix.user_field_size,
                            node_start_logical: envelope.node_start,
                            undecoded_node_bytes: envelope
                                .stream
                                .logical_bytes()
                                .len()
                                .saturating_sub(envelope.node_start),
                        }),
                    },
                    Err(error) => Inspection::ParseError { error },
                }
            } else {
                Inspection::Parsed {
                    header: header_report,
                    text: None,
                }
            }
        }
    };

    FileReport {
        path,
        bytes,
        sha256,
        result,
    }
}

fn header_report(header: &CommonHeader) -> HeaderReport {
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &header.entries {
        values
            .entry(entry.name.clone())
            .or_default()
            .push(display_bytes(&entry.value));
    }
    HeaderReport {
        body_offset: header.body_offset,
        values,
        diagnostics: header.diagnostics.clone(),
    }
}

fn display_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
