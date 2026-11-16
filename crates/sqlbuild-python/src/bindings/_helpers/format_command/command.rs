//! `sqb format`: rewrite authored SQL files in place and report the faults that remain.

use std::path::PathBuf;
use std::time::Instant;

use pyo3::exceptions::PyException;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::format_command::main::render_format_status::render_format_status;
use sqlbuild_core::format_command::main::render_lint_json::render_lint_json;
use sqlbuild_core::format_command::main::render_lint_text::render_lint_text;
use sqlbuild_core::format_command::models::LintReport;
use sqlbuild_rules::lint_text::main::scan_fixture_null_candidates::scan_fixture_null_candidates;

use crate::bindings::_helpers::format_command::config::{FormatConfig, format_config, lint_config};
use crate::bindings::_helpers::format_command::files::FormatChange;
use crate::bindings::_helpers::format_command::report::lint_report;
use crate::bindings::_helpers::format_command::run::{FormatOutcome, FormatRequest, run_format};
use crate::bindings::_helpers::format_command::selection::{LintInputs, resolve_lint_inputs};
use crate::bindings::_helpers::format_command::stages::quote_fixable_file_count;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;

const SQL_ANALYSIS_SETTING_KEY: &str = "sql_analysis";
const DEFAULT_DIALECT: &str = "generic";
const START_MESSAGE: &str = "Formatting SQL  START";
const ERROR_MESSAGE: &str = "Formatting SQL  ERROR";

/// The command's flags.
struct FormatFlags {
    check: bool,
    diff: bool,
    fixtures_only: bool,
    fix: bool,
    json_output: bool,
    use_color: bool,
    no_cache: bool,
}

/// The adapter, selected files and discovered inputs a run formats with.
struct Resolved<'py> {
    dialect: String,
    adapter: Option<Bound<'py, PyAny>>,
    selected: Option<std::collections::BTreeSet<PathBuf>>,
    discovered: Option<Bound<'py, PyAny>>,
    has_fixture_candidates: bool,
    /// Why `--fix` could not discover the project; descriptions that escape quotes may cause it.
    discovery_error: Option<PyErr>,
}

fn string_tuple(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    value.try_iter()?.map(|item| item?.extract()).collect()
}

fn write_stdout(py: Python<'_>, text: &str) -> PyResult<()> {
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (text,))?;
    Ok(())
}

/// Resolve selectors when the run needs the adapter, swallowing failures of the unscoped run.
fn resolve_inputs<'py>(
    project_dir: &Bound<'py, PyAny>,
    (select, exclude): (&[String], &[String]),
    (fix, dialect): (bool, &str),
) -> PyResult<Resolved<'py>> {
    let py: Python<'py> = project_dir.py();
    let scoped: bool = !select.is_empty() || !exclude.is_empty();
    let mut resolved: Resolved<'py> = Resolved {
        dialect: dialect.to_owned(),
        adapter: None,
        selected: None,
        discovered: None,
        has_fixture_candidates: false,
        discovery_error: None,
    };
    if !scoped {
        let directory: PathBuf = project_dir.extract()?;
        resolved.has_fixture_candidates = !scan_fixture_null_candidates(&directory, None)
            .map_err(value_error)?
            .paths
            .is_empty();
    }
    if scoped || resolved.has_fixture_candidates || fix {
        match resolve_lint_inputs(project_dir, (select, exclude)) {
            Ok(LintInputs {
                adapter,
                selected,
                discovered,
            }) => {
                resolved.has_fixture_candidates =
                    native_field(&discovered, "test_files")?.is_truthy()?;
                let adapter_dialect: Bound<'py, PyAny> =
                    native_field(&adapter, "sql_analysis_dialect_name")?;
                resolved.dialect = if adapter_dialect.is_truthy()? {
                    adapter_dialect.extract()?
                } else {
                    DEFAULT_DIALECT.to_owned()
                };
                resolved.adapter = Some(adapter);
                resolved.selected = selected;
                resolved.discovered = Some(discovered);
            }
            Err(error) if fix && !scoped && error.is_instance_of::<PyException>(py) => {
                resolved.discovery_error = Some(error);
                return Ok(resolved);
            }
            Err(error) if scoped || fix || !error.is_instance_of::<PyException>(py) => {
                return Err(error);
            }
            Err(_) => {}
        }
    }
    if fix {
        match discover_project_inputs(
            py,
            Some(keywords(py, &[("project_dir", project_dir.clone())])?),
        ) {
            Ok(discovered) => resolved.discovered = Some(discovered),
            Err(error) if error.is_instance_of::<PyException>(py) => {
                resolved.discovered = None;
                resolved.discovery_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(resolved)
}

/// The discovered inputs fixture-null fixes run with, when SQL analysis is enabled.
fn fixture_inputs<'py>(resolved: &Resolved<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(discovered) = &resolved.discovered else {
        return Ok(None);
    };
    if !resolved.has_fixture_candidates {
        return Ok(None);
    }
    let local: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let settings_owner: Bound<'py, PyAny> =
        if native_field(&local, "setting_overrides")?.contains(SQL_ANALYSIS_SETTING_KEY)? {
            local
        } else {
            native_field(discovered, "project_config")?
        };
    let enabled: bool = native_field(
        &native_field(&settings_owner, "settings")?,
        SQL_ANALYSIS_SETTING_KEY,
    )?
    .is_truthy()?;
    Ok(enabled.then(|| discovered.clone()))
}

/// Print stable unified diffs for the files the formatter would change.
fn render_diff(py: Python<'_>, changes: &[FormatChange<'_>]) -> PyResult<()> {
    let unified_diff: Bound<'_, PyAny> = py.import("difflib")?.getattr("unified_diff")?;
    for change in changes {
        let lines = |text: &str| -> PyResult<Bound<'_, PyAny>> {
            PyString::new(py, text).call_method1("splitlines", (true,))
        };
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("fromfile", change.path.str()?)?;
        keywords.set_item("tofile", change.path.str()?)?;
        let diff: Bound<'_, PyAny> = unified_diff.call(
            (lines(&change.before)?, lines(&change.after)?),
            Some(&keywords),
        )?;
        let joined: Bound<'_, PyAny> = PyString::new(py, "").call_method1("join", (diff,))?;
        write_stdout(py, &joined.extract::<String>()?)?;
    }
    Ok(())
}

fn stderr_status<'py>(py: Python<'py>, use_color: bool) -> PyResult<Bound<'py, PyAny>> {
    status_reporter(&py.import("sys")?.getattr("stderr")?, use_color)
}

fn format_with_status<'py>(
    py: Python<'py>,
    request: &FormatRequest<'_, 'py>,
    use_color: bool,
) -> PyResult<(FormatOutcome<'py>, Bound<'py, PyAny>, f64)> {
    let started: Instant = Instant::now();
    let status: Bound<'py, PyAny> = stderr_status(py, use_color)?;
    status.call_method1("start", (START_MESSAGE,))?;
    match run_format(request) {
        Ok(outcome) => Ok((outcome, status, started.elapsed().as_secs_f64())),
        Err(error) => {
            status.call_method1("error", (ERROR_MESSAGE,))?;
            Err(error)
        }
    }
}

fn complete_status(status: &Bound<'_, PyAny>, report: &LintReport, elapsed: f64) -> PyResult<()> {
    let (failed, message) = render_format_status(report, elapsed);
    if failed {
        status.call_method1("error", (message,))?;
    } else {
        call_with(
            &native_field(status, "complete")?,
            &[("message", PyString::new(status.py(), &message).into_any())],
        )?;
    }
    Ok(())
}

fn run_command(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
    selectors: (&[String], &[String]),
    flags: &FormatFlags,
) -> PyResult<i32> {
    let config: FormatConfig = format_config(py, project_dir)?;
    let resolved: Resolved<'_> = resolve_inputs(
        project_dir,
        selectors,
        (flags.fix, &config.settings.dialect),
    )?;
    if let Some(error) = &resolved.discovery_error {
        let directory: PathBuf = project_dir.extract()?;
        if quote_fixable_file_count(&directory).map_err(value_error)? == 0 {
            return Err(error.clone_ref(py));
        }
    }
    let lint: Bound<'_, PyAny> = lint_config(py, &config, &resolved.dialect)?;
    let fixture_discovered: Option<Bound<'_, PyAny>> = if flags.fix {
        resolved.discovered.clone()
    } else {
        fixture_inputs(&resolved)?
    };
    let request: FormatRequest<'_, '_> = FormatRequest {
        project_dir,
        config: &lint,
        adapter: resolved.adapter.as_ref(),
        selected: resolved.selected.as_ref(),
        discovered: fixture_discovered.as_ref(),
        fixtures_only: flags.fixtures_only,
        write: !(flags.check || flags.diff),
        fix: flags.fix,
        cache: !(flags.no_cache || flags.fix),
        discovery_error: resolved.discovery_error.as_ref(),
    };
    let (outcome, status, elapsed) = format_with_status(py, &request, flags.use_color)?;
    let root: PathBuf = project_dir.extract()?;
    let report: LintReport = lint_report(&outcome, &root)?;
    complete_status(&status, &report, elapsed)?;
    if flags.diff {
        render_diff(py, &outcome.changes)?;
    }
    let rendered: String = if flags.json_output {
        render_lint_json(&report).map_err(emit_error)?
    } else {
        let heading: &str = if flags.check || flags.diff {
            "Would format files:"
        } else {
            "Formatted files:"
        };
        render_lint_text(
            &report,
            CliStyle {
                use_color: flags.use_color,
            },
            heading,
        )
    };
    write_stdout(py, &rendered)?;
    let failed: bool =
        report.fault_count() > 0 || (flags.check && !report.formatted_files.is_empty());
    Ok(i32::from(failed))
}

/// Run one `sqb format` invocation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_format_command<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<i32> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_format_command",
        options,
        (
            &["project_dir"],
            &[
                "select",
                "exclude",
                "check",
                "diff",
                "fixtures_only",
                "fix",
                "json_output",
                "no_color",
                "no_cache",
            ],
        ),
    )?;
    let raw_project_dir: Bound<'py, PyAny> = arguments.value("project_dir")?;
    let project_dir: Bound<'py, PyAny> = if raw_project_dir.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        raw_project_dir
    };
    let use_color: bool = !arguments.flag("no_color")? && stdout_supports_color(py)?;
    let flags: FormatFlags = FormatFlags {
        check: arguments.flag("check")?,
        diff: arguments.flag("diff")?,
        fixtures_only: arguments.flag("fixtures_only")?,
        fix: arguments.flag("fix")?,
        json_output: arguments.flag("json_output")?,
        use_color,
        no_cache: arguments.flag("no_cache")?,
    };
    let select: Vec<String> = string_tuple(&arguments.value("select")?)?;
    let exclude: Vec<String> = string_tuple(&arguments.value("exclude")?)?;
    run_command(py, &project_dir, (&select, &exclude), &flags)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_format_command, module)?)?;
    Ok(())
}
