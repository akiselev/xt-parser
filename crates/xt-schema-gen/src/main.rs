use std::{env, fs, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xt-schema-gen: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        println!("Usage: xt-schema-gen INPUT.s_t OUTPUT_SCHEMA_DIRECTORY");
        return Ok(());
    }
    if arguments.len() != 2 {
        return Err("expected INPUT.s_t and OUTPUT_SCHEMA_DIRECTORY".to_owned());
    }
    let input_path = PathBuf::from(&arguments[0]);
    let output_path = PathBuf::from(&arguments[1]);
    let input =
        fs::read(&input_path).map_err(|error| format!("read {}: {error}", input_path.display()))?;
    let source_name = input_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 file name", input_path.display()))?;
    let generated = xt_schema_gen::generate_schema(&input, source_name)?;
    xt_schema_gen::materialize(&generated, &output_path)
}
