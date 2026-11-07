//! `sqlbuild`: the same command line as `sqb`, by running the `sqb` installed beside it without linking the library.

#[path = "../launcher/_helpers/handoff.rs"]
mod handoff;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use handoff::{beside, hand_off, process_exit};

fn main() -> ExitCode {
    let program: PathBuf = std::env::current_exe().unwrap_or_default();
    let sqb_name = format!("sqb{}", std::env::consts::EXE_SUFFIX);
    let Some(sqb) = beside(&program, &sqb_name) else {
        let _ = writeln!(
            std::io::stderr(),
            "sqlbuild: cannot find {sqb_name} beside {}; reinstall sqlbuild-cli",
            program.display()
        );
        return ExitCode::FAILURE;
    };
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    match hand_off(&sqb, &arguments) {
        Ok(Some(code)) => process_exit(code),
        Ok(None) => ExitCode::FAILURE,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "sqlbuild: cannot start {}: {error}",
                sqb.display()
            );
            ExitCode::FAILURE
        }
    }
}
