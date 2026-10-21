//! Print selected typed nodes of one SCH_30100 text file as JSON, for triage.
//!
//! Usage: xt-show FILE INDEX... | FILE --type NAME
//! (`--type` matches the generated node variant name, e.g. Halfedge.)

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: xt-show FILE INDEX... | FILE --type NAME")?;
    let rest: Vec<String> = args.collect();
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let document = xt_schema::parse_text_30100(&bytes).map_err(|e| e.to_string())?;
    let by_type = rest.first().is_some_and(|a| a == "--type");
    let wanted: Vec<u32> = if by_type {
        Vec::new()
    } else {
        rest.iter().filter_map(|a| a.parse().ok()).collect()
    };
    for (record, node) in document.raw.nodes.iter().zip(&document.nodes) {
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
