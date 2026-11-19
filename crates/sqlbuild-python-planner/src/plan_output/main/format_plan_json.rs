//! The plan as one JSON document, `json.dumps(indent=2)`.

use crate::plan_output::_helpers::plan_json::IGNORE_AUDIT_FAILURES_KEY;
use crate::plan_output::_helpers::plan_json::column_migrations_json;
use crate::plan_output::_helpers::plan_json::function_json;
use crate::plan_output::_helpers::plan_json::migrations_json;
use crate::plan_output::_helpers::plan_json::model_json;
use crate::plan_output::_helpers::plan_json::old_name_json;
use crate::plan_output::_helpers::plan_json::providers_json;
use crate::plan_output::_helpers::plan_json::python_entry_json;
use crate::plan_output::_helpers::plan_json::retention_json;
use crate::plan_output::_helpers::plan_json::seed_json;
use crate::plan_output::_helpers::plan_json::source_load_json;
use crate::plan_output::_helpers::plan_json::table_type_json;
use crate::plan_output::_helpers::plan_json::warning_json;
use crate::plan_output::_helpers::values::mapped;
use crate::plan_output::constants::{
    CHANGED_KEY, FULL_REFRESH_JOBS_KEY, IGNORE_AUDIT_FAILURES_JOBS_KEY, WAREHOUSE_JOBS_KEY,
};
use crate::plan_output::main::any::any;
use crate::plan_output::main::dict::dict;
use crate::plan_output::main::items::items;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_core::plan_display::constants::SYNCED_COPY_KEY;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::compile_outputs::main::indented_json::indented_json;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::compiled_project::constants::{
    JOB_FULL_REFRESH_KEY, JOB_IGNORED_AUDIT_FAILURES_KEY, JOB_WAREHOUSES_KEY,
};

/// The plan as one JSON document, `json.dumps(indent=2)`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn format_plan_json(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "format_plan_json",
        options,
        (
            &["plan"],
            &["python_plan_entries", SYNCED_COPY_KEY, CHANGED_KEY],
        ),
    )?;
    let plan: Bound<'_, PyAny> = arguments.value("plan")?;
    let python_entries: Vec<Bound<'_, PyAny>> = if arguments.passed("python_plan_entries")? {
        items(&arguments.value("python_plan_entries")?)?
    } else {
        Vec::new()
    };
    let length = |field: &str| -> PyResult<usize> { native_field(&plan, field)?.len() };
    let python_nodes: Vec<Bound<'_, PyAny>> = python_entries
        .iter()
        .map(python_entry_json)
        .collect::<PyResult<_>>()?;
    let result: Bound<'_, PyDict> = dict(
        py,
        vec![
            (
                "selected_count",
                any(
                    py,
                    length("model_entries")?
                        + length("seed_entries")?
                        + length("function_entries")?,
                )?,
            ),
            (
                "source_load_count",
                any(py, length("source_load_entries")?)?,
            ),
            ("python_node_count", any(py, python_nodes.len())?),
            (
                "models",
                mapped(&native_field(&plan, "model_entries")?, model_json)?,
            ),
            (
                "seeds",
                mapped(&native_field(&plan, "seed_entries")?, seed_json)?,
            ),
            (
                "source_loads",
                mapped(
                    &native_field(&plan, "source_load_entries")?,
                    source_load_json,
                )?,
            ),
            (
                "functions",
                mapped(&native_field(&plan, "function_entries")?, function_json)?,
            ),
            ("python_nodes", PyList::new(py, python_nodes)?.into_any()),
            ("providers", providers_json(&plan, &python_entries)?),
            (
                "warnings",
                mapped(&native_field(&plan, "warnings")?, warning_json)?,
            ),
            ("retention", retention_json(&plan)?),
            ("table_type_conversions", table_type_json(&plan)?),
            ("migrations", migrations_json(&plan)?),
            ("column_migrations", column_migrations_json(&plan)?),
            (
                "old_names",
                mapped(
                    &native_field(&plan, "old_name_view_entries")?,
                    old_name_json,
                )?,
            ),
        ],
    )?;
    let metadata: Bound<'_, PyAny> = native_field(&plan, "metadata")?;
    if metadata.is_truthy()? {
        result.set_item("metadata", &metadata)?;
    }
    let synced_copy: Bound<'_, PyAny> = arguments.value(SYNCED_COPY_KEY)?;
    if !synced_copy.is_none() {
        result.set_item(SYNCED_COPY_KEY, synced_copy)?;
    }
    if arguments.passed(CHANGED_KEY)? {
        result.set_item(CHANGED_KEY, arguments.value(CHANGED_KEY)?)?;
    }
    if native_field(&plan, IGNORE_AUDIT_FAILURES_KEY)?.is_truthy()? {
        result.set_item(IGNORE_AUDIT_FAILURES_KEY, true)?;
    }
    for (key, label) in [
        (
            JOB_IGNORED_AUDIT_FAILURES_KEY,
            IGNORE_AUDIT_FAILURES_JOBS_KEY,
        ),
        (JOB_FULL_REFRESH_KEY, FULL_REFRESH_JOBS_KEY),
    ] {
        let jobs: Bound<'_, PyAny> = mapping_get(&metadata, key)?;
        if !jobs.is_none() {
            result.set_item(
                label,
                PyList::new(py, jobs.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
            )?;
        }
    }
    let warehouses: Bound<'_, PyAny> = mapping_get(&metadata, JOB_WAREHOUSES_KEY)?;
    if !warehouses.is_none() {
        result.set_item(WAREHOUSE_JOBS_KEY, warehouses)?;
    }
    indented_json(result.as_any())
}
