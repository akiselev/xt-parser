use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode};

use serde::Serialize;
use xt_parser::{MachineEncoding, NodeRecord, TextEnvelope};
use xt_schema::{
    parse_binary_37102, parse_binary_37102_with_machine, parse_text_30100, parse_text_37102,
};

#[derive(Serialize)]
struct Report {
    text: PathBuf,
    text_schema: String,
    binaries: Vec<BinaryComparison>,
    node_count: usize,
    binary_node_type_censuses_equal: bool,
    text_binary_node_type_census_equal: bool,
}

#[derive(Serialize)]
struct BinaryComparison {
    path: PathBuf,
    kind: String,
    node_count: usize,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xt-format-compare: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if arguments.len() < 2 {
        return Err("usage: xt-format-compare <model-text.x_t> <model-binary.x_b>...".to_owned());
    }
    let text_input =
        fs::read(&arguments[0]).map_err(|error| format!("{}: {error}", arguments[0].display()))?;
    let text_envelope = TextEnvelope::parse(&text_input).map_err(|error| error.to_string())?;
    let text_nodes = if text_envelope
        .prefix
        .schema_name
        .windows(b"_30100_".len())
        .any(|window| window == b"_30100_")
    {
        parse_text_30100(&text_input)
            .map_err(|error| error.to_string())?
            .raw
            .nodes
    } else {
        parse_text_37102(&text_input)
            .map_err(|error| error.to_string())?
            .raw
            .nodes
    };
    let text_census = node_type_census(&text_nodes);
    let mut binary_census = None;
    let mut binary_node_type_censuses_equal = true;
    let mut text_binary_node_type_census_equal = true;
    let mut binaries = Vec::new();

    for path in &arguments[1..] {
        let input = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let header = xt_parser::CommonHeader::parse(&input).map_err(|error| error.to_string())?;
        let binary = if input.get(header.body_offset) == Some(&b'B') {
            parse_binary_37102_with_machine(&input, Some(MachineEncoding::LITTLE_IEEE_ASCII))
        } else {
            parse_binary_37102(&input)
        }
        .map_err(|error| format!("{}: {error}", path.display()))?;
        let census = node_type_census(&binary.raw.nodes);
        if let Some(baseline) = &binary_census {
            binary_node_type_censuses_equal &= baseline == &census;
        } else {
            binary_census = Some(census.clone());
        }
        text_binary_node_type_census_equal &= text_census == census;
        binaries.push(BinaryComparison {
            path: path.clone(),
            kind: format!("{:?}", binary.raw.envelope.prefix.kind),
            node_count: binary.nodes.len(),
        });
    }

    if !binary_node_type_censuses_equal {
        return Err("binary node-type censuses differ by encoding".to_owned());
    }
    if !text_binary_node_type_census_equal {
        return Err("text and binary node-type censuses differ".to_owned());
    }
    let report = Report {
        text: arguments[0].clone(),
        text_schema: String::from_utf8_lossy(&text_envelope.prefix.schema_name).into_owned(),
        binaries,
        node_count: text_nodes.len(),
        binary_node_type_censuses_equal,
        text_binary_node_type_census_equal,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn node_type_census(nodes: &[NodeRecord]) -> BTreeMap<u16, usize> {
    let mut output = BTreeMap::new();
    for node in nodes {
        *output.entry(node.node_type).or_default() += 1;
    }
    output
}
