//! The `--since` base snapshot: resolved from git, compiled in a child interpreter, cached per commit.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_analysis::definition_changes::constants::DEFINITION_SNAPSHOT_FORMAT;
use sqlbuild_analysis::definition_changes::main::definition_snapshot_json::definition_snapshot_json;
use sqlbuild_analysis::definition_changes::main::parse_definition_snapshot::parse_definition_snapshot;
use sqlbuild_analysis::definition_changes::models::DefinitionSnapshot;
use sqlbuild_core::git_revisions::constants::BASE_REF_ALIAS;
use sqlbuild_core::git_revisions::errors::GitRevisionError;
use sqlbuild_core::git_revisions::main::export_git_tree::export_git_tree;
use sqlbuild_core::git_revisions::main::resolve_git_base::resolve_git_base;
use sqlbuild_core::git_revisions::main::resolve_since_reference::resolve_since_reference;
use sqlbuild_core::git_revisions::main::short_commit::short_commit;
use sqlbuild_core::git_revisions::models::GitBase;
use sqlbuild_core::scenario_command::errors::ScenarioCommandError;

use crate::bindings::_helpers::change_selection::base_cache::{
    cache_key, prune_snapshots, work_directory_name,
};
use crate::bindings::_helpers::change_selection::base_compile::NOTES_EXTENSION;
use crate::bindings::_helpers::change_selection::environment_nodes::with_base_dependencies;
use crate::bindings::_helpers::scenario_command::setup::{cli_user_error, flush, write};
use sqlbuild_python_base::planner_identity::constants::COMPACT;
use sqlbuild_python_base::planner_identity::main::sorted_dumps::sorted_dumps;
use sqlbuild_python_compile::compile_command::main::invocation::interpreter_values::interpreter_values;
use sqlbuild_python_front::discovery::main::project::configured_git_base_ref::configured_git_base_ref;

const CACHE_DIRECTORY: [&str; 2] = ["target", "since"];
/// Where base compiles work when the project's cache directory cannot be created.
const TEMPORARY_WORK_ROOT: &str = "sqlbuild-since";
const LOCAL_CONFIG_FILE: &str = "sqlbuild_local.toml";
const PROJECT_CONFIG_FILE: &str = "sqlbuild_project.toml";
const BASE_COMPILE_FAILED_CODE: &str = "C484";
const FAILURE_TAIL_LINES: usize = 20;
/// Compiles the exported base tree in a separate interpreter; a failure prints only its error.
const BASE_COMPILE_PROGRAM: &str = concat!(
    "import json, sys\n",
    "from sqlbuild._native import write_base_definition_snapshot\n",
    "try:\n",
    "    write_base_definition_snapshot(project_dir=sys.argv[1], output_path=sys.argv[2],\n",
    "        selected_target=sys.argv[3] or None, cli_vars=json.loads(sys.argv[4]))\n",
    "except Exception as error:\n",
    "    code = getattr(error, 'code', None)\n",
    "    message = f'error[{code}]: {error}' if code else f'{type(error).__name__}: {error}'\n",
    "    print(message, file=sys.stderr)\n",
    "    sys.exit(1)\n",
);

/// The base revision and its definition snapshot.
pub(crate) struct BaseComparison {
    pub(crate) base: GitBase,
    pub(crate) snapshot: DefinitionSnapshot,
}

/// The inputs that decide what the base compile produces besides its commit.
pub(crate) struct BaseVariant {
    pub(crate) selected_target: Option<String>,
    /// `--vars` as sorted compact JSON.
    pub(crate) vars_json: String,
}

/// The base variant of a command's `--target` and `--vars`.
pub(crate) fn base_variant(
    selected_target: Option<String>,
    cli_vars: &Bound<'_, PyAny>,
) -> PyResult<BaseVariant> {
    Ok(BaseVariant {
        selected_target,
        vars_json: vars_json(cli_vars)?,
    })
}

pub(crate) fn git_error(py: Python<'_>, error: GitRevisionError) -> PyErr {
    cli_user_error(
        py,
        ScenarioCommandError {
            code: error.code,
            message: error.message,
            help: Some(error.help),
        },
    )
}

fn compile_failed(py: Python<'_>, base: &GitBase, detail: &str) -> PyErr {
    cli_user_error(
        py,
        ScenarioCommandError {
            code: BASE_COMPILE_FAILED_CODE,
            message: format!(
                "Cannot compile the project at merge base {} of '{}' and HEAD:\n{detail}",
                short_commit(&base.merge_base),
                base.reference
            ),
            help: Some(
                "--since compares with the project as it was at the merge base, so that \
                 revision must compile. Compare with a later ref whose project compiles, or run \
                 without --since to run everything"
                    .to_owned(),
            ),
        },
    )
}

fn io_failed(py: Python<'_>, base: &GitBase, error: &std::io::Error) -> PyErr {
    compile_failed(py, base, &error.to_string())
}

/// The last lines of a failed base compile's output.
fn failure_tail(output: &Output) -> String {
    let text: String = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(FAILURE_TAIL_LINES)..].join("\n")
}

fn vars_json(cli_vars: &Bound<'_, PyAny>) -> PyResult<String> {
    if cli_vars.is_none() {
        return Ok("{}".to_owned());
    }
    sorted_dumps(cli_vars, COMPACT, true)
}

/// Run the base compile of `tree` into `output` in a separate interpreter.
fn compile_base_tree(
    py: Python<'_>,
    base: &GitBase,
    (tree, output): (&Path, &Path),
    (selected_target, vars): (Option<&str>, &str),
) -> PyResult<()> {
    let [_, executable, _, _] = interpreter_values(py)?;
    let finished: std::io::Result<Output> = py.detach(|| {
        Command::new(&executable)
            .arg("-c")
            .arg(BASE_COMPILE_PROGRAM)
            .arg(tree)
            .arg(output)
            .arg(selected_target.unwrap_or_default())
            .arg(vars)
            .current_dir(tree)
            .stdin(Stdio::null())
            .output()
    });
    let finished: Output = finished.map_err(|error| io_failed(py, base, &error))?;
    if finished.status.success() && output.is_file() {
        return Ok(());
    }
    Err(compile_failed(py, base, &failure_tail(&finished)))
}

/// Export the base tree, carry over the untracked local config, and compile it.
fn compiled_base_snapshot(
    py: Python<'_>,
    (base, project_dir, work): (&GitBase, &Path, &Path),
    (selected_target, vars): (Option<&str>, &str),
) -> PyResult<(DefinitionSnapshot, String)> {
    let tree: PathBuf = work.join("project");
    let exported: bool = export_git_tree(base, &tree).map_err(|error| git_error(py, error))?;
    if !exported || !tree.join(PROJECT_CONFIG_FILE).is_file() {
        let empty: DefinitionSnapshot = DefinitionSnapshot {
            format: DEFINITION_SNAPSHOT_FORMAT,
            ..DefinitionSnapshot::default()
        };
        return Ok((empty, String::new()));
    }
    let local: PathBuf = project_dir.join(LOCAL_CONFIG_FILE);
    if local.is_file() && !tree.join(LOCAL_CONFIG_FILE).exists() {
        std::fs::copy(&local, tree.join(LOCAL_CONFIG_FILE))
            .map_err(|error| io_failed(py, base, &error))?;
    }
    let output: PathBuf = work.join("definitions.json");
    compile_base_tree(py, base, (&tree, &output), (selected_target, vars))?;
    let text: String =
        std::fs::read_to_string(&output).map_err(|error| io_failed(py, base, &error))?;
    let snapshot: DefinitionSnapshot = parse_definition_snapshot(&text)
        .ok_or_else(|| compile_failed(py, base, "the base compile wrote an unreadable snapshot"))?;
    Ok((snapshot, read_notes(py, base, &output)?))
}

/// The notes stored beside snapshot `path`, empty when there are none.
fn read_notes(py: Python<'_>, base: &GitBase, path: &Path) -> PyResult<String> {
    match std::fs::read_to_string(path.with_extension(NOTES_EXTENSION)) {
        Ok(notes) => Ok(notes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(io_failed(py, base, &error)),
    }
}

/// The notes cached beside snapshot `path`; unreadable notes are skipped with a warning.
fn cached_notes(path: &Path) -> String {
    let notes: PathBuf = path.with_extension(NOTES_EXTENSION);
    match std::fs::read_to_string(&notes) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            warn(&format!(
                "Could not read the cached notes at {}: {error}",
                notes.display()
            ));
            String::new()
        }
    }
}

/// Write one warning line to stderr, which machine-readable stdout never carries.
fn warn(message: &str) {
    let mut stderr: std::io::StderrLock<'_> = std::io::stderr().lock();
    match writeln!(stderr, "warning: {message}") {
        Ok(()) | Err(_) => {}
    }
}

/// The cache only saves time, so a failure to write it is a warning and the run continues.
fn warn_uncached(path: &Path, error: &std::io::Error) {
    warn(&format!(
        "Could not cache the base revision at {}: {error}; continuing without the cache",
        path.display()
    ));
}

/// A fresh work directory under `cache_dir`, or the system temporary directory without one.
fn work_directory(cache_dir: &Path, short: &str) -> std::io::Result<PathBuf> {
    let name: String = work_directory_name(short);
    let work: PathBuf = cache_dir.join(&name);
    match std::fs::create_dir_all(&work) {
        Ok(()) => Ok(work),
        Err(_unavailable) => {
            let fallback: PathBuf = std::env::temp_dir().join(TEMPORARY_WORK_ROOT).join(&name);
            std::fs::create_dir_all(&fallback)?;
            Ok(fallback)
        }
    }
}

/// Cache `snapshot` at `cached` with its `notes`, staging the snapshot so readers never see half.
fn store_snapshot(
    cached: &Path,
    (snapshot, notes): (&DefinitionSnapshot, &str),
) -> std::io::Result<()> {
    let notes_path: PathBuf = cached.with_extension(NOTES_EXTENSION);
    if notes.is_empty() {
        match std::fs::remove_file(&notes_path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    } else {
        std::fs::write(&notes_path, notes)?;
    }
    let staged: PathBuf = cached.with_extension(format!("{}.tmp", std::process::id()));
    let stored: std::io::Result<()> = std::fs::write(&staged, definition_snapshot_json(snapshot))
        .and_then(|()| std::fs::rename(&staged, cached));
    if stored.is_err() {
        match std::fs::remove_file(&staged) {
            Ok(()) | Err(_) => {}
        }
    }
    stored
}

/// Resolve `since` to its merge base and load, or compile and cache, the base snapshot.
pub(crate) fn base_comparison<'py>(
    (stream, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    since: &str,
    variant: &BaseVariant,
) -> PyResult<BaseComparison> {
    let py: Python<'py> = stream.py();
    let directory: PathBuf = project_dir.extract()?;
    let configured: Option<String> = if since.trim() == BASE_REF_ALIAS {
        configured_git_base_ref(project_dir)?
    } else {
        None
    };
    let reference: String = resolve_since_reference(since, configured.as_deref())
        .map_err(|error| git_error(py, error))?;
    let base: GitBase = py
        .detach(|| resolve_git_base(&directory, &reference))
        .map_err(|error| git_error(py, error))?;
    let short: &str = short_commit(&base.merge_base);
    write(
        stream,
        &format!("Comparing with {reference} (merge base {short})\n"),
    )?;
    let vars: &str = &variant.vars_json;
    let target: Option<&str> = variant.selected_target.as_deref();
    let local_config: String = match std::fs::read_to_string(directory.join(LOCAL_CONFIG_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(io_failed(py, &base, &error)),
    };
    let key: String = cache_key(&directory, (target, vars, &local_config))
        .map_err(|error| io_failed(py, &base, &error))?;
    let cache_dir: PathBuf = CACHE_DIRECTORY
        .iter()
        .fold(directory.clone(), |path, part| path.join(part));
    let cached: PathBuf = cache_dir.join(format!("{}-{}.json", base.merge_base, &key[..16]));
    let stored: Option<DefinitionSnapshot> = match std::fs::read_to_string(&cached) {
        Ok(text) => parse_definition_snapshot(&text),
        Err(_) => None,
    };
    if let Some(snapshot) = stored {
        write(stream, &format!("Using cached base revision {short}\n"))?;
        write(stream, &cached_notes(&cached))?;
        flush(stream)?;
        let snapshot: DefinitionSnapshot = with_base_dependencies(py, &base, &snapshot)?;
        return Ok(BaseComparison { base, snapshot });
    }
    write(stream, &format!("Compiling base revision {short}...\n"))?;
    flush(stream)?;
    let started: Instant = Instant::now();
    let work: PathBuf =
        work_directory(&cache_dir, short).map_err(|error| io_failed(py, &base, &error))?;
    let compiled: PyResult<(DefinitionSnapshot, String)> =
        compiled_base_snapshot(py, (&base, &directory, &work), (target, vars));
    let _ = std::fs::remove_dir_all(&work);
    let (snapshot, notes) = compiled?;
    write(stream, &notes)?;
    match store_snapshot(&cached, (&snapshot, &notes)) {
        Ok(()) => match prune_snapshots(&cache_dir) {
            Ok(()) => {}
            Err(error) => warn_uncached(&cache_dir, &error),
        },
        Err(error) => warn_uncached(&cached, &error),
    }
    write(
        stream,
        &format!(
            "Compiled base revision {short} ({:.2}s)\n",
            started.elapsed().as_secs_f64()
        ),
    )?;
    flush(stream)?;
    let snapshot: DefinitionSnapshot = with_base_dependencies(py, &base, &snapshot)?;
    Ok(BaseComparison { base, snapshot })
}
