//! Build and render the `sqb dbt` selection plan.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::main::dbt_interop_plan_warnings::dbt_interop_plan_warnings;
use sqlbuild_core::cli_text::main::format_dbt_interop_plan::format_dbt_interop_plan;
use sqlbuild_core::cli_text::main::format_dbt_interop_plan_json::format_dbt_interop_plan_json;
use sqlbuild_core::cli_text::models::{DbtInteropPlanText, DbtLsNodeText, DisplayLimit};

use crate::bindings::_helpers::plan_output::plan_text::format_plan;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::RecordId;

const BUILD_OPTIONS: (&[&str], &[&str]) = (
    &[
        "command",
        "dbt_command_argv",
        "dbt_ls_nodes",
        "sqlbuild_command_argvs",
        "selection",
    ],
    &[
        "dbt_required_selector_terms",
        "supplemental_dbt_command_argvs",
        "warnings",
        "sqlbuild_plan_output",
    ],
);
const RENDER_OPTIONS: (&[&str], &[&str]) =
    (&["plan", "json_output", "use_color"], &["display_options"]);

fn strings(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| item?.extract()).collect()
}

fn argvs(value: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<String>>> {
    value.try_iter()?.map(|argv| strings(&argv?)).collect()
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

fn reason_value(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(native_field(value, "value")?.extract()?))
}

fn ls_node(node: &Bound<'_, PyAny>) -> PyResult<DbtLsNodeText> {
    Ok(DbtLsNodeText {
        unique_id: native_field(node, "unique_id")?.extract()?,
        resource_type: optional_text(&native_field(node, "resource_type")?)?,
        name: optional_text(&native_field(node, "name")?)?,
        fqn: strings(&native_field(node, "fqn")?)?,
    })
}

fn plan_text(plan: &Bound<'_, PyAny>) -> PyResult<DbtInteropPlanText> {
    let selection: Bound<'_, PyAny> = native_field(plan, "selection")?;
    let anchor_ids: Bound<'_, PyAny> = native_field(&selection, "dbt_anchor_unique_ids_by_term")?;
    let empty: Bound<'_, PyTuple> = PyTuple::empty(plan.py());
    let mut anchors: Vec<(String, Vec<String>)> = Vec::new();
    for term in native_field(&selection, "dbt_anchor_terms")?.try_iter()? {
        let term: Bound<'_, PyAny> = term?;
        let unique_ids: Vec<String> = strings(&mapping_get_or(&anchor_ids, &term, &empty)?)?;
        anchors.push((term.extract()?, unique_ids));
    }
    let mut path_translations: Vec<(String, String)> = Vec::new();
    for pair in native_field(&selection, "path_translations")?.try_iter()? {
        path_translations.push(pair?.extract()?);
    }
    Ok(DbtInteropPlanText {
        command: native_field(plan, "command")?.getattr("value")?.extract()?,
        dbt_argv: strings(&native_field(plan, "dbt_command_argv")?)?,
        supplemental_dbt_argvs: argvs(&native_field(plan, "supplemental_dbt_command_argvs")?)?,
        dbt_nodes: native_field(plan, "dbt_selected_nodes")?
            .try_iter()?
            .map(|node| ls_node(&node?))
            .collect::<PyResult<_>>()?,
        dbt_selected_unique_ids: strings(&native_field(plan, "dbt_selected_unique_ids")?)?,
        dbt_required_unique_ids: strings(&native_field(&selection, "dbt_required_unique_ids")?)?,
        dbt_required_selector_terms: strings(&native_field(plan, "dbt_required_selector_terms")?)?,
        dbt_skip_reason: reason_value(&native_field(plan, "dbt_skip_reason")?)?,
        sqlbuild_argvs: argvs(&native_field(plan, "sqlbuild_command_argvs")?)?,
        sqlbuild_models: strings(&native_field(&selection, "sqlbuild_model_names")?)?,
        sqlbuild_skip_reason: reason_value(&native_field(plan, "sqlbuild_skip_reason")?)?,
        anchors,
        path_translations,
        warnings: strings(&native_field(plan, "warnings")?)?,
    })
}

fn tuple_of<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(value.py(), value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
}

fn tuple_of_tuples<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<Bound<'py, PyAny>> = value
        .try_iter()?
        .map(|item| tuple_of(item?))
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(value.py(), items)?.into_any())
}

fn empty_or<'py>(value: Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if value.is_none() {
        return PyTuple::empty(value.py()).into_any();
    }
    value
}

fn skip_reason<'py>(py: Python<'py>, has_work: bool, member: &str) -> PyResult<Bound<'py, PyAny>> {
    if has_work {
        return Ok(py.None().into_bound(py));
    }
    enum_member(py, RecordId::DbtInteropSkipReason, member)
}

/// Build a display-ready selection plan without dbt model state.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_dbt_interop_plan<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "build_dbt_interop_plan", options, BUILD_OPTIONS)?;
    let command: Bound<'py, PyAny> =
        enum_from_value(py, RecordId::DbtInteropCommand, arguments.value("command")?)?;
    let nodes: Bound<'py, PyAny> = tuple_of(arguments.value("dbt_ls_nodes")?)?;
    let mut selected: Vec<String> = Vec::new();
    for node in nodes.try_iter()? {
        selected.push(native_field(&node?, "unique_id")?.extract()?);
    }
    selected.sort();
    let selection: Bound<'py, PyAny> = arguments.value("selection")?;
    let dbt_has_work: bool =
        !selected.is_empty() || native_field(&selection, "dbt_required_unique_ids")?.is_truthy()?;
    let sqlbuild_has_work: bool = native_field(&selection, "sqlbuild_model_names")?.is_truthy()?;
    let warnings: Vec<String> = dbt_interop_plan_warnings(
        strings(&empty_or(arguments.value("warnings")?))?,
        dbt_has_work || sqlbuild_has_work,
    );
    new_record_with(
        py,
        RecordId::DbtInteropPlan,
        &[
            ("command", command),
            (
                "dbt_command_argv",
                tuple_of(arguments.value("dbt_command_argv")?)?,
            ),
            ("dbt_selected_nodes", nodes),
            (
                "dbt_selected_unique_ids",
                PyTuple::new(py, selected)?.into_any(),
            ),
            (
                "sqlbuild_command_argvs",
                tuple_of_tuples(arguments.value("sqlbuild_command_argvs")?)?,
            ),
            ("selection", selection),
            (
                "sqlbuild_plan_output",
                arguments.value("sqlbuild_plan_output")?,
            ),
            (
                "dbt_required_selector_terms",
                tuple_of(empty_or(arguments.value("dbt_required_selector_terms")?))?,
            ),
            (
                "supplemental_dbt_command_argvs",
                tuple_of_tuples(empty_or(arguments.value("supplemental_dbt_command_argvs")?))?,
            ),
            (
                "dbt_skip_reason",
                skip_reason(py, dbt_has_work, "NO_DBT_WORK")?,
            ),
            (
                "sqlbuild_skip_reason",
                skip_reason(py, sqlbuild_has_work, "NO_SQLBUILD_WORK")?,
            ),
            ("warnings", PyTuple::new(py, warnings)?.into_any()),
        ],
    )
}

fn display_limit(options: &Bound<'_, PyAny>) -> PyResult<DisplayLimit> {
    Ok(DisplayLimit {
        max_entries: native_field(options, "max_entries_per_section")?.extract()?,
        overflow_flag: native_field(options, "overflow_flag")?.extract()?,
    })
}

/// Render a dbt interop plan in the requested CLI output format.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_dbt_interop_plan(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "render_dbt_interop_plan", options, RENDER_OPTIONS)?;
    let plan: Bound<'_, PyAny> = arguments.value("plan")?;
    let text: DbtInteropPlanText = plan_text(&plan)?;
    if arguments.flag("json_output")? {
        return Ok(format_dbt_interop_plan_json(&text));
    }
    let mut display: Bound<'_, PyAny> = arguments.value("display_options")?;
    if !display.is_truthy()? {
        display = new_record(py, RecordId::DisplayOptions, (), None)?;
    }
    let use_color: bool = arguments.flag("use_color")?;
    let plan_output: Bound<'_, PyAny> = native_field(&plan, "sqlbuild_plan_output")?;
    let sqlbuild_plan: Option<String> =
        if text.sqlbuild_skip_reason.is_some() || plan_output.is_none() {
            None
        } else {
            Some(
                format_plan(
                    py,
                    Some(keywords(
                        py,
                        &[
                            ("plan", plan_output),
                            (
                                "use_color",
                                PyBool::new(py, use_color).to_owned().into_any(),
                            ),
                            ("display_options", display.clone()),
                        ],
                    )?),
                )
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
                .extract()?,
            )
        };
    Ok(format!(
        "\n{}",
        format_dbt_interop_plan(
            &text,
            sqlbuild_plan.as_deref(),
            (&display_limit(&display)?, use_color)
        )
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_dbt_interop_plan, module)?)?;
    module.add_function(wrap_pyfunction!(render_dbt_interop_plan, module)?)
}
