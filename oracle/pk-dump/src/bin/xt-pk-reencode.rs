//! Receive XT files into Parasolid and transmit each at several schema versions.
//!
//! Build for `x86_64-pc-windows-gnu` and run under Wine:
//!   xt-pk-reencode LIST OUT_DIR VERSION...
//! For input `N` in LIST and each VERSION, writes `OUT_DIR/NNNN.vVERSION.x_t`
//! (N is the zero-padded position in LIST) and prints one line per attempt.

use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use parasolid::{Behaviour, FrustrumConfig, Session, SessionConfig, fileio};
use parasolid_sys::{
    PK_PART_transmit, PK_PART_transmit_o_t, PK_transmit_format_text_c,
};

fn main() {
    if let Err(message) = run() {
        eprintln!("xt-pk-reencode: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let list = args.next().ok_or("usage: xt-pk-reencode LIST OUT_DIR VERSION...")?;
    let out_dir = PathBuf::from(args.next().ok_or("missing OUT_DIR")?);
    let versions: Vec<i32> = args
        .map(|a| a.parse().map_err(|_| format!("bad version {a}")))
        .collect::<Result<_, _>>()?;
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let paths: Vec<String> = fs::read_to_string(&list)
        .map_err(|e| format!("read {list}: {e}"))?
        .lines()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    let staging = std::env::temp_dir().join(format!("xt-pk-reencode-{}", std::process::id()));
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let _session = Session::start(
        SessionConfig::new()
            .behaviour(Behaviour::Latest)
            .frustrum(FrustrumConfig::new().base_dir(&staging))
            .check_arguments(true)
            .general_topology(true),
    )
    .map_err(|e| format!("session: {e}"))?;

    for (position, path) in paths.iter().enumerate() {
        let key = format!("in{position}");
        fs::copy(path, staging.join(format!("{key}.xmt_txt"))).map_err(|e| e.to_string())?;
        let bodies = match fileio::receive(&key) {
            Ok(bodies) => bodies,
            Err(error) => {
                println!("{path}\treceive\t{error}");
                continue;
            }
        };
        let tags: Vec<i32> = bodies.iter().map(|b| b.tag()).collect();
        for version in &versions {
            let out_key = format!("out{position}v{version}");
            let key_c = CString::new(out_key.clone()).map_err(|e| e.to_string())?;
            let mut options = PK_PART_transmit_o_t::default();
            options.transmit_format = PK_transmit_format_text_c;
            options.transmit_version = *version;
            // SAFETY: tags are live parts of this session; the key is a valid C string
            // and the options struct outlives the call.
            let rc = unsafe {
                PK_PART_transmit(tags.len() as i32, tags.as_ptr(), key_c.as_ptr(), &options)
            };
            let produced = staging.join(format!("{out_key}.xmt_txt"));
            if rc == 0 && produced.exists() {
                let target = out_dir.join(format!("{position:04}.v{version}.x_t"));
                fs::copy(&produced, &target).map_err(|e| e.to_string())?;
                println!("{path}\t{version}\tok\t{}", target.display());
            } else {
                println!("{path}\t{version}\terror\trc={rc}");
            }
        }
    }
    Ok(())
}
