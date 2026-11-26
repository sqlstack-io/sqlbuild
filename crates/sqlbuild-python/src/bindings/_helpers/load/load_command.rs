//! `sqb load`: build only the loads, loaders and sources the selection reaches, as `sqb build` loads them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::load_command::main::load_selectors::load_selectors;
use sqlbuild_core::load_command::main::render_empty_load::render_empty_load;
use sqlbuild_core::load_command::models::LoadKinds;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_front::discovery::models::SecretPolicy;
use sqlbuild_python_project::planner_selection::main::reject_unit_test_selectors::reject_unit_test_selectors;

use crate::bindings::_helpers::build_command::command::run_build_command;

/// The `sqb load` request fields the build takes as they are.
const PASSED_FIELDS: [&str; 8] = [
    "project_dir",
    "no_color",
    "selected_target",
    "exclude",
    "concurrency",
    "cursor_overrides",
    "cli_vars",
    "json_output_path",
];

/// The kinds of loading node `discovered` holds.
fn load_kinds(discovered: &Bound<'_, PyAny>) -> PyResult<LoadKinds> {
    let mut kinds = LoadKinds {
        loaders: native_field(discovered, "loader_functions")?.is_truthy()?,
        ..LoadKinds::default()
    };
    for file in native_field(discovered, "source_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        kinds.loads |= !native_field(&file, "load")?.is_none();
        kinds.sources |= native_field(&file, "source_entries")?.is_truthy()?;
    }
    Ok(kinds)
}

/// Run one `sqb load` request as a build of the loading nodes it selects, with tests and audits off, on the inputs it discovered.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_load_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let project_dir: Bound<'_, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'_, PyAny> = discover_project_inputs(
        py,
        Some(keywords(
            py,
            &[
                ("project_dir", project_dir.clone()),
                ("extract_output_column_locations", boolean(py, false)),
                (
                    "secret_policy",
                    PyString::new(py, SecretPolicy::Defer.name()).into_any(),
                ),
            ],
        )?),
    )?;
    let select: Bound<'_, PyAny> = native_field(request, "select")?;
    reject_unit_test_selectors(&select, &native_field(request, "exclude")?, &discovered)?;
    let selected: Vec<String> = select.extract()?;
    let selectors: Vec<String> = load_selectors(&selected, load_kinds(&discovered)?);
    if selectors.is_empty() {
        let machine: bool = native_field(request, "json_output")?.extract()?;
        let style = CliStyle {
            use_color: !native_field(request, "no_color")?.extract::<bool>()?
                && !machine
                && stdout_supports_color(py)?,
        };
        let stream: Bound<'_, PyAny> =
            py.import("sys")?
                .getattr(if machine { "stderr" } else { "stdout" })?;
        stream.call_method1("write", (format!("{}\n", render_empty_load(style)),))?;
        return Ok(0);
    }
    let mut fields: Vec<(&str, Bound<'_, PyAny>)> = vec![
        ("project_dir", project_dir),
        ("select", PyTuple::new(py, &selectors)?.into_any()),
        ("load_sources", boolean(py, true)),
        ("reload_sources", native_field(request, "reload")?),
        ("run_tests", boolean(py, false)),
        ("run_audits", boolean(py, false)),
        ("json_output", native_field(request, "json_output")?),
        ("command_name", text(py, "load")),
        ("discovered_inputs", discovered),
    ];
    for name in PASSED_FIELDS.iter().skip(1) {
        fields.push((name, native_field(request, name)?));
    }
    let build: Bound<'_, PyAny> = new_record_with(py, RecordId::BuildCommandRequest, &fields)?;
    run_build_command(py, &build)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_load_command, module)?)?;
    Ok(())
}
