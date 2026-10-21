//! Compare native and Parasolid digests file by file.
//!
//! Usage: xt-xcheck-compare NATIVE.ndjson ORACLE.ndjson OUT.tsv
//! Writes one TSV row per native file and prints a summary.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use xt_xcheck_digest::{FileDigest, compare::{Category, Verdict, compare_files}};

fn read(path: &str) -> Result<Vec<FileDigest>, String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("read {path}: {e}"))?
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{path}: {e}")))
        .collect()
}

fn main() {
    if let Err(message) = run() {
        eprintln!("xt-xcheck-compare: {message}");
        std::process::exit(2);
    }
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let native = read(&args.next().ok_or("usage: NATIVE ORACLE OUT")?)?;
    let oracle = read(&args.next().ok_or("missing ORACLE")?)?;
    let out = args.next().ok_or("missing OUT")?;
    let oracle: HashMap<&str, &FileDigest> = oracle.iter().map(|d| (d.path.as_str(), d)).collect();

    let mut tsv = std::io::BufWriter::new(
        std::fs::File::create(&out).map_err(|e| format!("create {out}: {e}"))?,
    );
    writeln!(tsv, "file\tverdict\tcategories\tdetail").map_err(|e| e.to_string())?;
    let (mut pass, mut fail, mut silent, mut native_error) = (0, 0, 0, 0);
    let mut by_category: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_silent: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_native: BTreeMap<String, usize> = BTreeMap::new();
    for file in &native {
        let verdict = compare_files(file, oracle.get(file.path.as_str()).copied());
        let (name, categories, detail) = match &verdict {
            Verdict::Pass => {
                pass += 1;
                ("pass", String::new(), String::new())
            }
            Verdict::Fail(list) => {
                fail += 1;
                let mut seen: Vec<Category> = list.iter().map(|(c, _)| *c).collect();
                seen.sort();
                seen.dedup();
                for category in &seen {
                    *by_category.entry(category.name()).or_insert(0) += 1;
                }
                (
                    "fail",
                    seen.iter().map(|c| c.name()).collect::<Vec<_>>().join(","),
                    list.iter()
                        .map(|(c, d)| format!("{}: {}", c.name(), d))
                        .collect::<Vec<_>>()
                        .join(" | "),
                )
            }
            Verdict::OracleSilent { stage, message } => {
                silent += 1;
                *by_silent.entry(format!("{stage}: {}", first_words(message))).or_insert(0) += 1;
                ("oracle_silent", stage.clone(), message.clone())
            }
            Verdict::NativeError { stage, message } => {
                native_error += 1;
                *by_native.entry(format!("{stage}: {}", first_words(message))).or_insert(0) += 1;
                ("native_error", stage.clone(), message.clone())
            }
        };
        writeln!(tsv, "{}\t{}\t{}\t{}", clean(&file.path), name, categories, clean(&detail))
            .map_err(|e| e.to_string())?;
    }
    tsv.flush().map_err(|e| e.to_string())?;

    println!("files\t{}", native.len());
    println!("pass\t{pass}");
    println!("fail\t{fail}");
    println!("oracle_silent\t{silent}");
    println!("native_error\t{native_error}");
    for (category, count) in &by_category {
        println!("fail.{category}\t{count}");
    }
    for (reason, count) in &by_silent {
        println!("oracle_silent.{reason}\t{count}");
    }
    for (reason, count) in &by_native {
        println!("native_error.{reason}\t{count}");
    }
    Ok(())
}

fn first_words(message: &str) -> String {
    message.split_whitespace().take(8).collect::<Vec<_>>().join(" ")
}
