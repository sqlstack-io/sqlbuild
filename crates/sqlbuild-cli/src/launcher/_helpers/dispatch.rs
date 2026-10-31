//! Decide how one invocation runs: from a launch record, natively, or in the Python CLI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::launcher::_helpers::handoff::{hand_off, python_cli};
use crate::launcher::_helpers::launch_key::{launch_key, launch_record_path};
use crate::launcher::_helpers::messages::{print_version, report_missing_python_cli};
use crate::launcher::constants::{
    HANDOFF_FAILURE_EXIT_CODE, LAUNCH_RECORD_OPTION, VERSION_OPTIONS,
};
use crate::native_commands::main::parse_native_invocation::parse_native_invocation;
use crate::native_commands::main::run_control_invocation::run_control_invocation;
use crate::native_commands::main::run_location_invocation::run_location_invocation;
use crate::native_commands::main::run_native_invocation::run_native_invocation;
use crate::native_commands::models::NativeParse;
use crate::replay::main::serve_reused_compile::serve_reused_compile;

/// The launch record path and key for these arguments, when the directory is known.
fn launch_record(
    program: &Path,
    script: &Path,
    arguments: &[OsString],
) -> Option<(PathBuf, String)> {
    let Ok(cwd) = std::env::current_dir() else {
        return None;
    };
    let key = launch_key(program, script, arguments, &cwd);
    launch_record_path(arguments, &cwd, &key).map(|path| (path, key))
}

fn exit_code(code: i64) -> i32 {
    i32::try_from(code).unwrap_or(HANDOFF_FAILURE_EXIT_CODE)
}

/// Run one invocation started at `started` and return its exit code.
pub(crate) fn launch(arguments: Vec<OsString>, started: (Instant, SystemTime)) -> i32 {
    if let [only] = arguments.as_slice()
        && VERSION_OPTIONS.iter().any(|option| only == option)
    {
        print_version();
        return 0;
    }
    let native = parse_native_invocation(&arguments);
    if let Ok(NativeParse::Exit(code)) = &native {
        return *code;
    }
    if let Ok(NativeParse::Invocation { namespace, .. }) = &native
        && let Some(code) = run_control_invocation(namespace)
    {
        return code;
    }
    let program: PathBuf = std::env::current_exe().unwrap_or_default();
    let Some(script) = python_cli(&program) else {
        report_missing_python_cli(&program);
        return HANDOFF_FAILURE_EXIT_CODE;
    };
    if let Ok(NativeParse::Invocation { namespace, .. }) = &native
        && let Some(code) = run_location_invocation(namespace, &script)
    {
        return code;
    }
    let mut handed: Vec<OsString> = Vec::with_capacity(arguments.len() + 1);
    let record: Option<(PathBuf, String)> = launch_record(&program, &script, &arguments);
    if let Some((path, key)) = &record
        && let Some(code) = serve_reused_compile(path, key, started)
    {
        return exit_code(code);
    }
    if let Ok(NativeParse::Invocation {
        namespace,
        use_color,
    }) = native
        && let Ok(code) = run_native_invocation(&namespace, &script, use_color)
    {
        return code;
    }
    if let Some((path, _)) = record {
        let mut option = OsString::from(LAUNCH_RECORD_OPTION);
        option.push(path.as_os_str());
        handed.push(option);
    }
    handed.extend(arguments);
    hand_off(&script, &handed)
}
