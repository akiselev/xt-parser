//! Check that embedded-schema decoding agrees with exact-schema decoding.
//!
//! Usage: xt-embedded-check LIST
//! LIST holds one `.x_t` path per line; files must be SCH_30100 or SCH_37102
//! in the embedded-edits form.
//! Prints one line per file and a summary; exits 1 if any file disagrees.

fn main() -> Result<(), String> {
    let list = std::env::args()
        .nth(1)
        .ok_or("usage: xt-embedded-check LIST")?;
    let mut agree = 0usize;
    let mut disagree = 0usize;
    for path in std::fs::read_to_string(&list)
        .map_err(|e| e.to_string())?
        .lines()
    {
        if path.is_empty() {
            continue;
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let embedded = xt_schema::parse_text_embedded(&bytes);
        let exact = xt_schema::parse_text(&bytes).map(|d| d.raw().clone());
        match (embedded, exact) {
            (Ok(a), Ok(b)) if a.nodes == b.nodes && a.terminator == b.terminator => agree += 1,
            (a, b) => {
                disagree += 1;
                println!(
                    "{path}\tDISAGREE\tembedded={:?}\texact={:?}",
                    a.map(|d| d.nodes.len()).map_err(|e| e.to_string()),
                    b.map(|d| d.nodes.len()).map_err(|e| e.to_string())
                );
            }
        }
    }
    println!("agree\t{agree}\ndisagree\t{disagree}");
    if disagree > 0 {
        std::process::exit(1);
    }
    Ok(())
}
