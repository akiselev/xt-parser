//! Describe XT files with the native parser and write one JSON digest per line.
//!
//! Usage: xt-native-dump LIST OUT [--threads N]
//! LIST holds one `.x_t` path per line.

use std::io::Write;
use std::sync::Mutex;

use xt_xcheck_digest::{FileDigest, Outcome};

mod v30100 {
    use xt_schema::schemas::sch_30100::Node;
    use xt_schema::view::sch_30100 as view;
    include!("describe.rs");
}

mod v37102 {
    use xt_schema::schemas::sch_37102::Node;
    use xt_schema::view::sch_37102 as view;
    include!("describe.rs");
}

fn main() {
    if let Err(message) = run() {
        eprintln!("xt-native-dump: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let list = args
        .next()
        .ok_or("usage: xt-native-dump LIST OUT [--threads N]")?;
    let out = args.next().ok_or("missing OUT")?;
    let mut threads = 4usize;
    while let Some(flag) = args.next() {
        if flag == "--threads" {
            threads = args
                .next()
                .and_then(|v| v.parse().ok())
                .filter(|n| *n > 0)
                .ok_or("--threads needs a positive integer")?;
        } else {
            return Err(format!("unknown argument {flag}"));
        }
    }
    let paths: Vec<String> = std::fs::read_to_string(&list)
        .map_err(|e| format!("read {list}: {e}"))?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let results: Mutex<Vec<Option<FileDigest>>> = Mutex::new(vec![None; paths.len()]);
    std::thread::scope(|scope| {
        for worker in 0..threads {
            let (paths, results) = (&paths, &results);
            scope.spawn(move || {
                for index in (worker..paths.len()).step_by(threads) {
                    let digest = describe_file(&paths[index]);
                    if let Ok(mut guard) = results.lock() {
                        guard[index] = Some(digest);
                    }
                }
            });
        }
    });
    let results = results.into_inner().map_err(|e| e.to_string())?;
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(&out).map_err(|e| format!("create {out}: {e}"))?,
    );
    for digest in results.into_iter().flatten() {
        let line = serde_json::to_string(&digest).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())
}

fn describe_file(path: &str) -> FileDigest {
    let failed = |stage: &str, message: String| FileDigest {
        path: path.to_owned(),
        outcome: Outcome::Failed {
            stage: stage.to_owned(),
            message,
        },
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return failed("read", error.to_string()),
    };
    let described = match xt_schema::parse_text(&bytes) {
        Ok(xt_schema::TextDocument::Sch30100(document)) => v30100::describe(&document),
        Ok(xt_schema::TextDocument::Sch37102(document)) => v37102::describe(&document),
        Err(error) => return failed("parse", error.to_string()),
    };
    match described {
        Ok(bodies) => FileDigest {
            path: path.to_owned(),
            outcome: Outcome::Described { bodies },
        },
        Err((stage, message)) => failed(stage, message),
    }
}
