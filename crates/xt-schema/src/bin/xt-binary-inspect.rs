use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode};

use serde::Serialize;
use xt_parser::{BinaryKind, MachineEncoding};
use xt_schema::{parse_binary_37102, parse_binary_37102_with_machine};

#[derive(Serialize)]
struct Report {
    path: PathBuf,
    kind: BinaryKind,
    machine: MachineEncoding,
    schema: String,
    input_bytes: usize,
    raw_nodes: usize,
    typed_nodes: usize,
    node_types: BTreeMap<u16, usize>,
    unresolved_pointer_indices: usize,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xt-binary-inspect: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut bare_little_ieee_ascii = false;
    let mut path = None;
    for argument in env::args_os().skip(1) {
        if argument == "--bare-little-ieee-ascii" {
            bare_little_ieee_ascii = true;
        } else if path.replace(PathBuf::from(argument)).is_some() {
            return Err(
                "usage: xt-binary-inspect [--bare-little-ieee-ascii] <model.x_b>".to_owned(),
            );
        }
    }
    let path = path.ok_or_else(|| {
        "usage: xt-binary-inspect [--bare-little-ieee-ascii] <model.x_b>".to_owned()
    })?;
    let input = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let document = if bare_little_ieee_ascii {
        parse_binary_37102_with_machine(&input, Some(MachineEncoding::LITTLE_IEEE_ASCII))
    } else {
        parse_binary_37102(&input)
    }
    .map_err(|error| error.to_string())?;
    let mut node_types = BTreeMap::new();
    for node in &document.raw.nodes {
        *node_types.entry(node.node_type).or_default() += 1;
    }
    let report = Report {
        path,
        kind: document.raw.envelope.prefix.kind,
        machine: document.raw.envelope.prefix.machine,
        schema: String::from_utf8_lossy(&document.raw.envelope.prefix.schema_name).into_owned(),
        input_bytes: input.len(),
        raw_nodes: document.raw.nodes.len(),
        typed_nodes: document.nodes.len(),
        node_types,
        unresolved_pointer_indices: document.raw.unresolved_pointer_indices.len(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}
