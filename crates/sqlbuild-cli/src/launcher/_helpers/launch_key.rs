//! The launch key: a digest of every process fact the Python CLI's reuse identity is built from.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sqlbuild_core::process_environment::main::environment_vars_os::environment_vars_os;

use crate::launcher::constants::{
    LAUNCH_KEY_DOMAIN, MISSING_FILE_STAMP, PROJECT_DIR_ABBREVIATION_PREFIXES, PROJECT_DIR_OPTIONS,
    VOLATILE_ENVIRONMENT,
};
use crate::replay::constants::{LAUNCH_RECORD_DIRECTORY, LAUNCH_RECORD_SUFFIX};

#[cfg(unix)]
fn os_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    value.to_string_lossy().into_owned().into_bytes()
}

fn update(hasher: &mut blake3::Hasher, field: &[u8]) {
    let _ = hasher.update(&(field.len() as u64).to_le_bytes());
    let _ = hasher.update(field);
}

fn file_stamp(path: &Path) -> Vec<u8> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return MISSING_FILE_STAMP.to_vec();
    };
    let modified: u128 = match metadata
        .modified()
        .map(|time| time.duration_since(UNIX_EPOCH))
    {
        Ok(Ok(elapsed)) => elapsed.as_nanos(),
        _ => 0,
    };
    format!("{}:{modified}", metadata.len()).into_bytes()
}

fn is_volatile(name: &std::ffi::OsStr) -> bool {
    VOLATILE_ENVIRONMENT
        .iter()
        .any(|volatile| name == *volatile)
}

/// The digest of the program, its Python handoff, arguments, directory, environment and terminals.
pub(crate) fn launch_key(
    program: &Path,
    python_cli: &Path,
    arguments: &[OsString],
    cwd: &Path,
) -> String {
    let mut hasher = blake3::Hasher::new();
    update(&mut hasher, LAUNCH_KEY_DOMAIN.as_bytes());
    update(&mut hasher, env!("CARGO_PKG_VERSION").as_bytes());
    for path in [program, python_cli] {
        update(&mut hasher, &os_bytes(path.as_os_str()));
        update(&mut hasher, &file_stamp(path));
    }
    if let Ok(script) = std::fs::read(python_cli) {
        update(&mut hasher, &script);
    }
    update(&mut hasher, &(arguments.len() as u64).to_le_bytes());
    for argument in arguments {
        update(&mut hasher, &os_bytes(argument));
    }
    update(&mut hasher, &os_bytes(cwd.as_os_str()));
    let mut environment: Vec<(OsString, OsString)> = environment_vars_os()
        .into_iter()
        .filter(|(name, _)| !is_volatile(name))
        .collect();
    environment.sort();
    for (name, value) in &environment {
        update(&mut hasher, &os_bytes(name));
        update(&mut hasher, &os_bytes(value));
    }
    let terminals = [
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
        std::io::stderr().is_terminal(),
    ];
    update(&mut hasher, &terminals.map(u8::from));
    hasher.finalize().to_hex().to_string()
}

/// The project directory the arguments name, or `None` when argparse might read them otherwise.
fn named_project_dir(arguments: &[OsString]) -> Option<Option<OsString>> {
    let mut named: Option<OsString> = None;
    let mut index = 0;
    while index < arguments.len() {
        let text = arguments[index].to_string_lossy();
        if PROJECT_DIR_OPTIONS.contains(&text.as_ref()) {
            named = Some(arguments.get(index + 1)?.clone());
            index += 2;
            continue;
        }
        if let Some((option, value)) = text.split_once('=')
            && PROJECT_DIR_OPTIONS.contains(&option)
        {
            named = Some(OsString::from(value));
        } else if PROJECT_DIR_ABBREVIATION_PREFIXES
            .iter()
            .any(|prefix| text.starts_with(prefix))
        {
            return None;
        }
        index += 1;
    }
    Some(named)
}

/// Where the Python CLI keeps this invocation's launch record, if the project directory is plain.
pub(crate) fn launch_record_path(arguments: &[OsString], cwd: &Path, key: &str) -> Option<PathBuf> {
    let project = match named_project_dir(arguments)? {
        Some(named) => cwd.join(named),
        None => cwd.to_path_buf(),
    };
    let mut path = project;
    for part in LAUNCH_RECORD_DIRECTORY {
        path.push(part);
    }
    path.push(format!("{key}{LAUNCH_RECORD_SUFFIX}"));
    Some(path)
}
