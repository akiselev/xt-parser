//! Print selected typed nodes of one text file (SCH_30100 or SCH_37102) as JSON.
//!
//! Usage: xt-show FILE INDEX... | FILE --type NAME
//! (`--type` matches the generated node variant name, e.g. Halfedge.)

use serde::Serialize;
use xt_parser::NodeRecord;
use xt_schema::TextDocument;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: xt-show FILE INDEX... | FILE --type NAME")?;
    let rest: Vec<String> = args.collect();
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    match xt_schema::parse_text(&bytes).map_err(|e| e.to_string())? {
        TextDocument::Sch30100(d) => show(&d.raw.nodes, &d.nodes, &rest),
        TextDocument::Sch37102(d) => show(&d.raw.nodes, &d.nodes, &rest),
    }
}

fn show<N: Serialize>(records: &[NodeRecord], nodes: &[N], rest: &[String]) -> Result<(), String> {
    let by_type = rest.first().is_some_and(|a| a == "--type");
    let wanted: Vec<u32> = rest.iter().filter_map(|a| a.parse().ok()).collect();
    for (record, node) in records.iter().zip(nodes) {
        let selected = if by_type {
            let value = serde_json::to_value(node).map_err(|e| e.to_string())?;
            value.get("node").and_then(|n| n.as_str()) == rest.get(1).map(String::as_str)
        } else {
            wanted.contains(&record.file_index)
        };
        if selected {
            println!(
                "{}",
                serde_json::to_string(node).map_err(|e| e.to_string())?
            );
        }
    }
    Ok(())
}
