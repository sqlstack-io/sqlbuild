//! `sqb dbt plan|run|build|debug`: dbt interop through the SQLBuild twin project.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyInt};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::dbt_command::main::dbt_output_options::dbt_output_options;
use sqlbuild_core::dbt_command::models::DbtOutputOptions;

use crate::bindings::_helpers::build_progress::progress_reporters::PlanningProgressReporter;
use crate::bindings::_helpers::dbt_command::auto_init::{args_tuple, ensure_twin_project};
use crate::bindings::_helpers::dbt_command::debug::run_dbt_debug;
use crate::bindings::_helpers::dbt_command::support::{color_enabled, effective_dir, flag, stream};
use crate::bindings::_helpers::dbt_interop::plan::render_dbt_interop_plan;
use crate::bindings::_helpers::dbt_interop::runtime_config::validate_dbt_execution_args;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;

const PLAN_COMMAND: &str = "plan";
const DEBUG_COMMAND: &str = "debug";
const BOUNDED_SECTION_ENTRIES: i64 = 10;

/// Progress goes to stderr under `--json` so stdout stays machine-readable.
struct Progress<'py> {
    stream: Bound<'py, PyAny>,
    use_color: bool,
    reporter: Bound<'py, PyAny>,
}

impl<'py> Progress<'py> {
    fn new(py: Python<'py>, options: &DbtOutputOptions, no_color: bool) -> PyResult<Self> {
        let use_color: bool = !no_color && !options.json_output && color_enabled(py)?;
        let stream: Bound<'py, PyAny> = stream(
            py,
            if options.json_output {
                "stderr"
            } else {
                "stdout"
            },
        )?;
        let reporter: Bound<'py, PyAny> = PlanningProgressReporter::new(
            py,
            &stream.clone(),
            flag(py, use_color).extract::<bool>()?,
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?;
        Ok(Self {
            stream,
            use_color,
            reporter,
        })
    }

    fn blank_line(&self) -> PyResult<()> {
        self.stream.call_method1("write", ("\n",))?;
        self.stream.call_method0("flush")?;
        Ok(())
    }
}

/// `sqb dbt plan`: the combined dbt and SQLBuild plan, as text or JSON.
fn run_plan(project_dir: &Bound<'_, PyAny>, args: &[String], no_color: bool) -> PyResult<i32> {
    let py: Python<'_> = project_dir.py();
    let options: DbtOutputOptions = dbt_output_options(args);
    let progress: Progress<'_> = Progress::new(py, &options, no_color)?;
    if !options.json_output {
        progress.blank_line()?;
    }
    let plan: Bound<'_, PyAny> = call_with(
        &native_attr(py, NativeAttr::PlanDbtInteropFromProject)?,
        &[
            ("project_dir", project_dir.clone()),
            ("args", args_tuple(py, &options.routed_args)?),
            (
                "on_progress",
                native_field(&progress.reporter, "on_progress")?,
            ),
            ("progress_stream", progress.stream.clone()),
            ("use_color", flag(py, progress.use_color)),
        ],
    )?;
    let display: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::DisplayOptions,
        &[(
            "max_entries_per_section",
            if options.verbose {
                py.None().into_bound(py)
            } else {
                PyInt::new(py, BOUNDED_SECTION_ENTRIES).into_any()
            },
        )],
    )?;
    let rendered: String = render_dbt_interop_plan(
        py,
        Some(keywords(
            py,
            &[
                ("plan", plan),
                ("json_output", flag(py, options.json_output)),
                ("use_color", flag(py, progress.use_color)),
                ("display_options", display),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .extract()?;
    stream(py, "stdout")?.call_method1("write", (format!("{rendered}\n"),))?;
    Ok(0)
}

/// `sqb dbt run|build`: dbt first, then the SQLBuild work the combined graph selects.
fn run_execution(
    command: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    args: &[String],
    no_color: bool,
) -> PyResult<i32> {
    let py: Python<'_> = project_dir.py();
    let options: DbtOutputOptions = dbt_output_options(args);
    let progress: Progress<'_> = Progress::new(py, &options, no_color)?;
    progress.blank_line()?;
    let request: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::DbtInteropExecutionRequest,
        &[
            ("command", command.clone()),
            ("project_dir", project_dir.clone()),
            ("args", args_tuple(py, &options.routed_args)?),
            (
                "on_progress",
                native_field(&progress.reporter, "on_progress")?,
            ),
            ("progress_stream", progress.stream.clone()),
            ("dbt_stdout_stream", stream(py, "stdout")?),
            ("use_color", flag(py, progress.use_color)),
            ("verbose", flag(py, options.verbose)),
            ("json_output", flag(py, options.json_output)),
        ],
    )?;
    native_attr(py, NativeAttr::ExecuteDbtInteropFromProject)?
        .call1((request,))?
        .extract()
}

/// Execute one `sqb dbt` interop command.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_dbt_command(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<i32> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "run_dbt_command",
        options,
        (&["command", "project_dir", "args", "no_color"], &[]),
    )?;
    let command: Bound<'_, PyAny> = arguments.value("command")?;
    let args_value: Bound<'_, PyAny> = arguments.value("args")?;
    let no_color: bool = arguments.flag("no_color")?;
    validate_dbt_execution_args(&command, &args_value)?;
    let args: Vec<String> = args_value
        .try_iter()?
        .map(|arg| arg?.extract())
        .collect::<PyResult<_>>()?;
    let (project_dir, args) = ensure_twin_project(
        effective_dir(py, Some(arguments.value("project_dir")?))?,
        args,
        no_color,
    )?;
    let command_name: String = native_field(&command, "value")?.extract()?;
    match command_name.as_str() {
        PLAN_COMMAND => run_plan(&project_dir, &args, no_color),
        DEBUG_COMMAND => run_dbt_debug(&project_dir, &args, no_color),
        _ => run_execution(&command, &project_dir, &args, no_color),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_dbt_command, module)?)?;
    Ok(())
}
