//! Plan one `sqb dbt` command from compiled inputs: dbt listings, anchors, selection and argvs.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::dbt_cli::errors::DbtCliError;
use sqlbuild_core::dbt_cli::models::{DbtCliOptionValues, DbtInteropCommandKind};
use sqlbuild_core::dbt_plan::main::dbt_anchor_term_check::dbt_anchor_term_check;
use sqlbuild_core::dbt_plan::main::dbt_failure_detail::dbt_failure_detail;
use sqlbuild_core::dbt_plan::main::dbt_plan_argvs::dbt_plan_argvs;
use sqlbuild_core::dbt_plan::main::dbt_required_selector_terms::dbt_required_selector_terms;
use sqlbuild_core::dbt_plan::models::{DbtPlanArgvRequest, DbtPlanArgvs, DbtReferencedModel};

use crate::bindings::_helpers::dbt_interop::cli::{option_values, raised};
use crate::bindings::_helpers::dbt_interop::graph::{
    PlannerResolver, planner_resolver, resolve_dbt_interop_sqlbuild_selection, texts,
};
use crate::bindings::_helpers::dbt_interop::manifest::resolve_sqlbuild_model_dbt_refs;
use crate::bindings::_helpers::dbt_interop::plan::build_dbt_interop_plan;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const PLAN_OPTIONS: (&[&str], &[&str]) = (
    &[
        "command",
        "project",
        "manifest",
        "graph",
        "dbt_runner",
        "dbt_options",
        "command_args",
    ],
    &[],
);

/// `dbt_runner.ls(...)` result, raising when dbt ls failed.
fn listed<'py>(
    (runner, dbt_options): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    select: Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = runner.py();
    let result: Bound<'py, PyAny> = call_with(
        &native_field(runner, "ls")?,
        &[
            ("options", dbt_options.clone()),
            ("select", select),
            ("exclude", exclude.clone()),
        ],
    )?;
    let command: Bound<'py, PyAny> = native_field(&result, "command")?;
    if native_field(&command, "returncode")?.extract::<i64>()? == 0 {
        return Ok(result);
    }
    let detail: Option<String> = dbt_failure_detail(
        &native_field(&command, "stderr")?.extract::<String>()?,
        &native_field(&command, "stdout")?.extract::<String>()?,
    );
    let help: Bound<'py, PyAny> = detail.map_or_else(
        || py.None().into_bound(py),
        |detail| PyString::new(py, &detail).into_any(),
    );
    let error: Bound<'py, PyAny> = error_class(py, ErrorId::DbtInteropRuntime)?
        .call(("dbt ls failed",), Some(&keywords(py, &[("help", help)])?))?;
    Err(PyErr::from_value(error))
}

fn unique_ids<'py>(listing: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let mut ids: Vec<Bound<'py, PyAny>> = Vec::new();
    for node in native_field(listing, "nodes")?.try_iter()? {
        ids.push(native_field(&node?, "unique_id")?);
    }
    PyTuple::new(listing.py(), ids)
}

fn command_kind(command: &Bound<'_, PyAny>) -> PyResult<DbtInteropCommandKind> {
    let value: String = native_field(command, "value")?.extract()?;
    match value.as_str() {
        "plan" => Ok(DbtInteropCommandKind::Plan),
        "run" => Ok(DbtInteropCommandKind::Run),
        "build" => Ok(DbtInteropCommandKind::Build),
        "debug" => Ok(DbtInteropCommandKind::Debug),
        _ => Err(raised(
            command.py(),
            ErrorId::DbtInteropArgument,
            DbtCliError {
                message: format!("Unsupported sqb dbt command: {value}"),
                ..DbtCliError::default()
            },
        )),
    }
}

fn required_terms(
    arguments: &KeywordArguments<'_>,
    selection: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    let required: Vec<String> = texts(&native_field(selection, "dbt_required_unique_ids")?)?;
    if required.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = selection.py();
    let mut referenced: Vec<DbtReferencedModel> = Vec::new();
    for pair in resolve_sqlbuild_model_dbt_refs(
        py,
        arguments.value("project")?,
        arguments.value("manifest")?,
        Some(native_field(selection, "sqlbuild_model_names")?),
    )?
    .try_iter()?
    {
        let (_model, dbt_model): (Bound<'_, PyAny>, Bound<'_, PyAny>) = pair?.extract()?;
        referenced.push(DbtReferencedModel {
            unique_id: native_field(&dbt_model, "unique_id")?.extract()?,
            fqn: texts(&native_field(&dbt_model, "fqn")?)?,
            name: native_field(&dbt_model, "name")?.extract()?,
        });
    }
    Ok(dbt_required_selector_terms(&referenced, &required))
}

fn argv_tuples<'py>(py: Python<'py>, argvs: Vec<Vec<String>>) -> PyResult<Bound<'py, PyAny>> {
    let tuples: Vec<Bound<'py, PyTuple>> = argvs
        .into_iter()
        .map(|argv| PyTuple::new(py, argv))
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(py, tuples)?.into_any())
}

/// Plan dbt and SQLBuild work from already-compiled interop inputs.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn plan_dbt_interop_command<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "plan_dbt_interop_command", options, PLAN_OPTIONS)?;
    let command: Bound<'py, PyAny> =
        enum_from_value(py, RecordId::DbtInteropCommand, arguments.value("command")?)?;
    let command_args: Bound<'py, PyAny> = arguments.value("command_args")?;
    let (select, exclude) = (
        native_field(&command_args, "select")?,
        native_field(&command_args, "exclude")?,
    );
    let (runner, dbt_options) = (
        arguments.value("dbt_runner")?,
        arguments.value("dbt_options")?,
    );
    let full_listing: Bound<'py, PyAny> =
        listed((&runner, &dbt_options), select.clone(), &exclude)?;
    let anchors: Bound<'py, PyDict> = PyDict::new(py);
    let resolver: PlannerResolver<'py> = planner_resolver(arguments.value("project")?)?;
    for term in texts(&select)? {
        if !dbt_anchor_term_check(&resolver, &term)? {
            continue;
        }
        let only_term: Bound<'py, PyAny> = PyTuple::new(py, [&term])?.into_any();
        let listing: Bound<'py, PyAny> = listed((&runner, &dbt_options), only_term, &exclude)?;
        anchors.set_item(&term, unique_ids(&listing)?)?;
    }
    let selection: Bound<'py, PyAny> = resolve_dbt_interop_sqlbuild_selection(
        py,
        Some(keywords(
            py,
            &[
                ("project", arguments.value("project")?),
                ("manifest", arguments.value("manifest")?),
                ("graph", arguments.value("graph")?),
                ("select", select.clone()),
                ("exclude", exclude.clone()),
                ("dbt_anchor_unique_ids_by_term", anchors.into_any()),
            ],
        )?),
    )?;
    let required_selector_terms: Vec<String> = required_terms(&arguments, &selection)?;
    let options: DbtCliOptionValues = option_values(&dbt_options)?;
    let (dbt_executable, sqlbuild_executable): (String, String) = (
        native_field(&command_args, "dbt_executable")?.extract()?,
        native_field(&command_args, "sqlbuild_executable")?.extract()?,
    );
    let (dbt_command_args, sqlbuild_command_args, selected_model_names) = (
        texts(&native_field(&command_args, "dbt_command_args")?)?,
        texts(&native_field(&command_args, "sqlbuild_command_args")?)?,
        texts(&native_field(&selection, "sqlbuild_model_names")?)?,
    );
    let argvs: DbtPlanArgvs = dbt_plan_argvs(&DbtPlanArgvRequest {
        command: command_kind(&command)?,
        dbt_executable: &dbt_executable,
        sqlbuild_executable: &sqlbuild_executable,
        options: &options,
        dbt_command_args: &dbt_command_args,
        sqlbuild_command_args: &sqlbuild_command_args,
        selected_model_names: &selected_model_names,
        required_selector_terms: &required_selector_terms,
    });
    build_dbt_interop_plan(
        py,
        Some(keywords(
            py,
            &[
                ("command", command),
                (
                    "dbt_command_argv",
                    PyTuple::new(py, argvs.dbt_command_argv)?.into_any(),
                ),
                ("dbt_ls_nodes", native_field(&full_listing, "nodes")?),
                (
                    "sqlbuild_command_argvs",
                    argv_tuples(py, argvs.sqlbuild_command_argvs)?,
                ),
                ("selection", selection),
                (
                    "dbt_required_selector_terms",
                    PyTuple::new(py, required_selector_terms)?.into_any(),
                ),
                (
                    "supplemental_dbt_command_argvs",
                    argv_tuples(py, argvs.supplemental_dbt_command_argvs)?,
                ),
            ],
        )?),
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(plan_dbt_interop_command, module)?)
}
