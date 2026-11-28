//! `sqb rename` and `sqb mv`: plan from compiler facts, verify in a scratch copy, then write.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::lineage_command::models::LineageKey;
use sqlbuild_core::cli_text::main::format_count_noun::format_count_noun;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_refactor::refactor_command::main::compare_with_baseline::compare_with_baseline;
use sqlbuild_refactor::refactor_command::main::layer_move_suggestion::layer_move_suggestion;
use sqlbuild_refactor::refactor_command::main::refactor_pairs::refactor_pairs;
use sqlbuild_refactor::refactor_command::main::refactor_requests::refactor_requests;
use sqlbuild_refactor::refactor_command::main::render_project_failure_json::render_project_failure_json;
use sqlbuild_refactor::refactor_command::main::render_refactor_json::render_refactor_json;
use sqlbuild_refactor::refactor_command::main::render_refactor_text::render_refactor_text;
use sqlbuild_refactor::refactor_command::models::{
    BaselineComparison, LayerMove, PairArguments, RefactorDiagnostic, RefactorStatus,
    RefactorTargetRequest,
};
use sqlbuild_refactor::refactoring::errors::{RefactorError, RefactorErrorKind};
use sqlbuild_refactor::refactoring::main::commit_refactor_plan::commit_refactor_plan;
use sqlbuild_refactor::refactoring::main::plan_refactor::plan_refactor;
use sqlbuild_refactor::refactoring::main::stage_refactor_plan::stage_refactor_plan;
use sqlbuild_refactor::refactoring::main::with_model_migration::with_model_migration;
use sqlbuild_refactor::refactoring::models::{
    DeclarationPlacement, ModelFacts, RefactorFacts, RefactorOperation, RefactorPlan,
    RefactorRequest,
};
use sqlbuild_refactor::refactoring::types::Originals;

use crate::bindings::_helpers::refactor::offline_compile::{OfflineCompile, compile_for_refactor};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_compile::lineage::main::lineage_key::lineage_key;
use sqlbuild_python_front::refactor::main::declaration_placement::declaration_placement;
use sqlbuild_python_front::refactor::main::model_facts::model_facts;
use sqlbuild_python_front::refactor::main::refactor_facts::refactor_facts;
use sqlbuild_python_front::refactor::main::unit_test_rebindings::unit_test_rebindings;

const STAGING_PREFIX: &str = "sqb-refactor-";
const MOVE_COMMAND: &str = "mv";
const EDIT_ERROR_CODE: &str = "C957";
const WRITE_ERROR_CODE: &str = "C958";
const MOVE_FILE_ERROR_CODE: &str = "C954";
const RULE_PHASE: &str = "rule";
const PROJECT_FAILED_HELP: &str = "sqb compile";

fn imported<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import(module)?.getattr(name)
}

/// `CliUserError(message, code=..., help=...)`.
fn user_error(py: Python<'_>, (message, code, help): (String, &str, Option<String>)) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &message,
        (Some(code), help.as_deref()),
    )
}

/// The Python exception a native refactoring failure becomes.
fn refactor_error(py: Python<'_>, error: RefactorError) -> PyErr {
    let class = |name: ErrorId| -> PyErr {
        match error_class(py, name).and_then(|class| class.call1((error.message.clone(),))) {
            Ok(raised) => PyErr::from_value(raised),
            Err(failure) => failure,
        }
    };
    match error.kind {
        RefactorErrorKind::Input => user_error(py, (error.message, &error.code, error.help)),
        RefactorErrorKind::Edit => user_error(py, (error.message, EDIT_ERROR_CODE, None)),
        RefactorErrorKind::Write => user_error(py, (error.message, WRITE_ERROR_CODE, error.help)),
        RefactorErrorKind::Value => class(ErrorId::RefactorValue),
        RefactorErrorKind::Io => class(ErrorId::RefactorIO),
        RefactorErrorKind::Internal => native_compiler_error(error.message),
    }
}

/// One offline compile: its project facts and errors.
struct Compiled<'py> {
    project: Bound<'py, PyAny>,
    errors: Vec<Bound<'py, PyAny>>,
}

fn compile<'py>(py: Python<'py>, project_dir: &Path, no_cache: bool) -> PyResult<Compiled<'py>> {
    let compiled: OfflineCompile<'py> = compile_for_refactor(py, project_dir, no_cache)?;
    Ok(Compiled {
        project: compiled.project,
        errors: compiled.errors,
    })
}

fn optional_posix(path: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if path.is_none() {
        return Ok(None);
    }
    Ok(Some(path.call_method0("as_posix")?.extract()?))
}

/// A compiled project's errors, paths inside `root` made relative to it.
fn relative_diagnostics(
    errors: &[Bound<'_, PyAny>],
    root: &Path,
) -> PyResult<Vec<RefactorDiagnostic>> {
    let mut diagnostics: Vec<RefactorDiagnostic> = Vec::new();
    for error in errors {
        let path: Option<String> = optional_posix(&native_field(error, "path")?)?.map(|path| {
            let candidate: &Path = Path::new(&path);
            match candidate.strip_prefix(root) {
                Ok(relative) if candidate.is_absolute() => {
                    relative.to_string_lossy().replace('\\', "/")
                }
                _ => path,
            }
        });
        let phase: String = native_field(error, "phase")?.str()?.extract()?;
        diagnostics.push(RefactorDiagnostic {
            code: native_field(error, "code")?.extract()?,
            message: native_field(error, "message")?.extract()?,
            path,
            line: native_field(error, "line")?.extract()?,
            rule: phase == RULE_PHASE,
        });
    }
    Ok(diagnostics)
}

/// Print each compile error to stderr as `error[code] path: message`.
fn write_errors(py: Python<'_>, errors: &[Bound<'_, PyAny>]) -> PyResult<()> {
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    for error in errors {
        let location: String = optional_posix(&native_field(error, "path")?)?
            .map_or_else(String::new, |path| format!("{path}: "));
        let code: String = native_field(error, "code")?.str()?.extract()?;
        let message: String = native_field(error, "message")?.str()?.extract()?;
        stderr.call_method1("write", (format!("error[{code}] {location}{message}\n"),))?;
    }
    Ok(())
}

fn files(plan: &RefactorPlan) -> String {
    format_count_noun(
        i64::try_from(plan.changes.len()).unwrap_or(i64::MAX),
        "file",
        None,
    )
}

/// The Python transient status reporter on the command's progress stream.
struct Status<'py> {
    reporter: Bound<'py, PyAny>,
}

impl Status<'_> {
    fn start(&self, message: &str) -> PyResult<()> {
        self.reporter.call_method1("start", (message,))?;
        Ok(())
    }

    fn complete(&self, message: &str) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(self.reporter.py());
        options.set_item("message", message)?;
        self.reporter.call_method("complete", (), Some(&options))?;
        Ok(())
    }

    fn error(&self, message: &str) -> PyResult<()> {
        self.reporter.call_method1("error", (message,))?;
        Ok(())
    }

    fn close(&self) -> PyResult<()> {
        self.reporter.call_method0("close")?;
        Ok(())
    }
}

/// The command's options and output settings.
struct Command<'py> {
    request: Bound<'py, PyAny>,
    project_dir: PathBuf,
    use_color: bool,
    json_output: bool,
}

/// Where a refactoring stopped: plan, status, blocking errors and the rule-failure baseline.
struct Outcome {
    plan: RefactorPlan,
    status: RefactorStatus,
    diagnostics: Vec<RefactorDiagnostic>,
    baseline: BaselineComparison,
}

/// How the command ended: with a plan, or refused because the project does not compile.
enum Finished {
    Planned(Box<Outcome>),
    ProjectFailed(Vec<RefactorDiagnostic>),
}

/// A plan verified in the scratch copy, with its originals and the edited project's errors.
struct Verified {
    plan: RefactorPlan,
    originals: Originals,
    diagnostics: Vec<RefactorDiagnostic>,
}

fn target_lookup(all_keys: &Bound<'_, PyAny>) -> PyResult<HashMap<String, LineageKey>> {
    let mut names: HashMap<String, LineageKey> = HashMap::new();
    for item in mapping_items(all_keys)?.try_iter()? {
        let (name, key): (String, Bound<'_, PyAny>) = item?.extract()?;
        names.insert(name, lineage_key(&key)?);
    }
    Ok(names)
}

fn move_command(request: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(native_field(request, "command")?
        .str()?
        .extract::<String>()?
        == MOVE_COMMAND)
}

/// The target of each refactoring the command asks for: every move or rename pair.
fn target_requests(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<Vec<RefactorTargetRequest>> {
    let is_move: bool = move_command(request)?;
    let target: Option<String> = native_field(request, "target")?.extract()?;
    let second: Option<String> =
        native_field(request, if is_move { "destination" } else { "new_name" })?.extract()?;
    let cascade: bool = native_field(request, "cascade")?.extract()?;
    let more: Vec<String> = native_field(request, "more_moves")?.extract()?;
    let from_file: Option<String> = native_field(request, "from_file")?.extract()?;
    let contents: Option<String> = match &from_file {
        None => None,
        Some(path) => Some(std::fs::read_to_string(path).map_err(|error| {
            let noun: &str = if is_move { "move" } else { "rename" };
            user_error(
                py,
                (
                    format!("cannot read {noun} file {path}: {error}"),
                    MOVE_FILE_ERROR_CODE,
                    None,
                ),
            )
        })?),
    };
    let mut words: Vec<&str> = target.iter().chain(&second).map(String::as_str).collect();
    words.extend(more.iter().map(String::as_str));
    let arguments: PairArguments<'_> = PairArguments {
        is_move,
        words,
        file: from_file.as_deref().zip(contents.as_deref()),
    };
    let pairs: Vec<(String, String)> =
        refactor_pairs(&arguments).map_err(|error| refactor_error(py, error))?;
    Ok(pairs
        .into_iter()
        .map(|(target, second)| {
            let (new_name, destination) = if is_move {
                (None, Some(second))
            } else {
                (Some(second), None)
            };
            RefactorTargetRequest {
                is_move,
                target,
                new_name,
                destination,
                cascade,
            }
        })
        .collect())
}

fn plan(
    py: Python<'_>,
    project: &Bound<'_, PyAny>,
    requests: &[RefactorRequest],
) -> PyResult<RefactorPlan> {
    let facts: RefactorFacts = refactor_facts(project, requests)?;
    let moving: BTreeSet<&str> = requests
        .iter()
        .filter(|_| requests.len() > 1)
        .map(|request| request.model_name.as_str())
        .collect();
    let vacated: BTreeSet<String> = facts
        .models
        .iter()
        .filter(|model| moving.contains(model.name.as_str()))
        .map(|model| model.path.clone())
        .collect();
    let host_error: RefCell<Option<PyErr>> = RefCell::new(None);
    let host = |moves: &[(String, String)]| -> Result<DeclarationPlacement, RefactorError> {
        declaration_placement(project, moves, &vacated).map_err(|error| {
            host_error.replace(Some(error));
            RefactorError::internal("the declaration-move host failed")
        })
    };
    let test_host = |path: &str, destination: &str| -> Result<Option<Vec<String>>, RefactorError> {
        unit_test_rebindings(project, (path, destination)).map_err(|error| {
            host_error.replace(Some(error));
            RefactorError::internal("the unit-test scope host failed")
        })
    };
    let planned: Result<RefactorPlan, RefactorError> =
        plan_refactor(&facts, requests, &host, &test_host);
    if let Some(error) = host_error.into_inner() {
        return Err(error);
    }
    planned.map_err(|error| refactor_error(py, error))
}

fn stage(
    py: Python<'_>,
    (project_dir, staging_dir): (&Path, &Path),
    plan: &RefactorPlan,
    copy_inputs: bool,
) -> PyResult<Originals> {
    stage_refactor_plan(project_dir, staging_dir, plan, copy_inputs)
        .map_err(|error| refactor_error(py, error))
}

/// Verify the plan in a scratch copy, adding model migrations when the relation moves.
fn verify(
    py: Python<'_>,
    command: &Command<'_>,
    (before, mut plan): (&Bound<'_, PyAny>, RefactorPlan),
    (staging_dir, status): (&Path, &Status<'_>),
) -> PyResult<Result<Verified, RefactorPlan>> {
    let project_dir: &Path = &command.project_dir;
    status.start("Verifying the edited project compiles...")?;
    let mut originals: Originals = stage(py, (project_dir, staging_dir), &plan, true)?;
    let mut after: Compiled<'_> = compile(py, staging_dir, true)?;
    if plan.request.operation != RefactorOperation::RenameColumn {
        let before_facts: Vec<ModelFacts> = model_facts(before)?.unwrap_or_default();
        let after_facts: Option<Vec<ModelFacts>> = model_facts(&after.project)?;
        let migrated: RefactorPlan =
            with_model_migration(&plan, &before_facts, after_facts.as_deref(), &originals);
        if !migrated.blocking.is_empty() {
            status.error("Refused: the renamed model's history cannot be kept.")?;
            return Ok(Err(migrated));
        }
        if migrated.migrations != plan.migrations {
            plan = migrated;
            originals = stage(py, (project_dir, staging_dir), &plan, false)?;
            after = compile(py, staging_dir, true)?;
        }
    }
    let diagnostics: Vec<RefactorDiagnostic> = relative_diagnostics(&after.errors, staging_dir)?;
    Ok(Ok(Verified {
        plan,
        originals,
        diagnostics,
    }))
}

fn finished(outcome: Outcome) -> Finished {
    Finished::Planned(Box::new(outcome))
}

fn refused(plan: RefactorPlan, baseline: BaselineComparison) -> Finished {
    finished(Outcome {
        plan,
        status: RefactorStatus::Refused,
        diagnostics: Vec::new(),
        baseline,
    })
}

fn run(py: Python<'_>, command: &Command<'_>, status: &Status<'_>) -> PyResult<Finished> {
    let targets: Vec<RefactorTargetRequest> = target_requests(py, &command.request)?;
    let started: Instant = Instant::now();
    status.start("Compiling project...")?;
    let before: Compiled<'_> = compile(py, &command.project_dir, false)?;
    let existing: Vec<RefactorDiagnostic> =
        relative_diagnostics(&before.errors, &command.project_dir)?;
    if before.project.is_none() || existing.iter().any(|diagnostic| !diagnostic.rule) {
        status.error(&project_failed_message(command)?)?;
        write_errors(py, &before.errors)?;
        return Ok(Finished::ProjectFailed(existing));
    }
    let elapsed: f64 = started.elapsed().as_secs_f64();
    status.complete(&if existing.is_empty() {
        format!("Compiled project. ({elapsed:.2}s)")
    } else {
        format!(
            "Compiled project: {}; the change may not add or worsen any. ({elapsed:.2}s)",
            format_count_noun(
                i64::try_from(existing.len()).unwrap_or(i64::MAX),
                "existing rule failure",
                None,
            )
        )
    })?;
    let unplanned: BaselineComparison = compare_with_baseline(&existing, &existing, &[]);
    let names: HashMap<String, LineageKey> = target_lookup(&native_field(
        &native_field(&before.project, "graph")?,
        "all_keys",
    )?)?;
    let lookup = |name: &str| names.get(name).cloned();
    let requests: Vec<RefactorRequest> =
        refactor_requests(&targets, &lookup).map_err(|error| refactor_error(py, error))?;
    status.start("Planning edits...")?;
    let planned: RefactorPlan = plan(py, &before.project, &requests)?;
    status.complete(&format!("Planned edits to {}.", files(&planned)))?;
    if !planned.blocking.is_empty() || !planned.manual.is_empty() {
        status.error("Refused: some references cannot be rewritten safely.")?;
        return Ok(refused(planned, unplanned));
    }
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("prefix", STAGING_PREFIX)?;
    let staging: Bound<'_, PyAny> =
        imported(py, "tempfile", "TemporaryDirectory")?.call((), Some(&options))?;
    let staging_dir: PathBuf = native_field(&staging, "name")?.extract()?;
    let verified = verify(
        py,
        command,
        (&before.project, planned),
        (&staging_dir, status),
    );
    staging.call_method0("cleanup")?;
    let Verified {
        plan,
        originals,
        diagnostics,
    } = match verified? {
        Ok(verified) => verified,
        Err(migrated) => return Ok(refused(migrated, unplanned)),
    };
    let baseline: BaselineComparison =
        compare_with_baseline(&existing, &diagnostics, &plan.changes);
    if !baseline.blocking.is_empty() {
        status.error(
            if baseline.blocking.iter().all(|diagnostic| diagnostic.rule) {
                "Verification failed: the change adds or worsens rule failures."
            } else {
                "Verification failed: the edited project does not compile."
            },
        )?;
        return Ok(finished(Outcome {
            plan,
            status: RefactorStatus::CompileFailed,
            diagnostics: baseline.blocking.clone(),
            baseline,
        }));
    }
    status.complete(if baseline.before.is_empty() {
        "Verified: the edited project compiles."
    } else {
        "Verified: the edited project compiles and adds or worsens no rule failure."
    })?;
    if native_field(&command.request, "dry_run")?.extract::<bool>()? {
        return Ok(finished(Outcome {
            plan,
            status: RefactorStatus::DryRun,
            diagnostics: Vec::new(),
            baseline,
        }));
    }
    status.start(&format!("Writing {}...", files(&plan)))?;
    commit_refactor_plan(&command.project_dir, &originals, &plan)
        .map_err(|error| refactor_error(py, error))?;
    status.complete(&format!("Wrote {}.", files(&plan)))?;
    Ok(finished(Outcome {
        plan,
        status: RefactorStatus::Applied,
        diagnostics: Vec::new(),
        baseline,
    }))
}

/// The refusal for a project that does not compile before the change.
fn project_failed_message(command: &Command<'_>) -> PyResult<String> {
    let verb: &str = if move_command(&command.request)? {
        "moving"
    } else {
        "renaming"
    };
    Ok(format!(
        "Project does not compile; fix it before {verb} ({PROJECT_FAILED_HELP})"
    ))
}

fn write_stdout(py: Python<'_>, output: String) -> PyResult<()> {
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (output,))?;
    Ok(())
}

fn finish_project_failed(
    py: Python<'_>,
    command: &Command<'_>,
    diagnostics: &[RefactorDiagnostic],
) -> PyResult<i32> {
    if command.json_output {
        let message: String = project_failed_message(command)?;
        let output: String =
            render_project_failure_json((&message, PROJECT_FAILED_HELP), diagnostics)
                .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
        write_stdout(py, output)?;
    }
    Ok(1)
}

fn finish(py: Python<'_>, command: &Command<'_>, outcome: &Outcome) -> PyResult<i32> {
    let output: String = if command.json_output {
        render_refactor_json(
            &outcome.plan,
            (outcome.status, &outcome.diagnostics, &outcome.baseline),
        )
        .map_err(|error| PyValueError::new_err(format!("{error:?}")))?
    } else {
        let project_dir: Bound<'_, PyAny> = native_field(&command.request, "project_dir")?;
        let project_dir: Option<String> = if project_dir.is_none() {
            None
        } else {
            Some(project_dir.str()?.extract()?)
        };
        let layer_move: Option<LayerMove> = layer_move_suggestion(
            &outcome.plan,
            &outcome.diagnostics,
            (project_dir.as_deref(), cfg!(windows)),
        );
        render_refactor_text(
            &outcome.plan,
            (outcome.status, &outcome.diagnostics, &outcome.baseline),
            layer_move.as_ref(),
            CliStyle {
                use_color: command.use_color,
            },
        )
    };
    write_stdout(py, output)?;
    Ok(i32::from(!matches!(
        outcome.status,
        RefactorStatus::Applied | RefactorStatus::DryRun
    )))
}

/// Plan, verify in a scratch copy, and write one rename or move.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_refactor_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let project_dir: Bound<'_, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let json_output: bool = native_field(request, "json_output")?.extract()?;
    let use_color: bool =
        !native_field(request, "no_color")?.extract::<bool>()? && stdout_supports_color(py)?;
    let command: Command<'_> = Command {
        request: request.clone(),
        project_dir: project_dir.call_method0("resolve")?.extract()?,
        use_color,
        json_output,
    };
    let stream: Bound<'_, PyAny> =
        py.import("sys")?
            .getattr(if json_output { "stderr" } else { "stdout" })?;
    let status: Status<'_> = Status {
        reporter: status_reporter(&stream, use_color)?,
    };
    let result: PyResult<Finished> = compiler_guard(|| run(py, &command, &status));
    status.close()?;
    match result? {
        Finished::ProjectFailed(diagnostics) => finish_project_failed(py, &command, &diagnostics),
        Finished::Planned(outcome) => finish(py, &command, &outcome),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_refactor_command, module)?)?;
    Ok(())
}
