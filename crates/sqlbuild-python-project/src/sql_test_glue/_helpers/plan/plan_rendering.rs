//! Adapter rendering and declared cursor-window boundaries for native test planning.

use crate::compiled_views::main::compiled_models::name_object::name_object;
use crate::compiled_views::main::compiled_models::query_sql::query_sql;
use crate::sql_test_glue::_helpers::plan::plan_values::{model_payload, string};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;

use crate::sql_test_glue::_helpers::inputs::cursor_window::{
    declared_window_cursor_models, declares_cursor_window, render_test_cursor_intrinsics,
};
use crate::sql_test_glue::constants::SENTINEL;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) fn mentions(sql: &str) -> bool {
    ["__cursor_start", "__cursor_end"]
        .iter()
        .any(|name| sql.contains(name))
}

pub(crate) fn models<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = project.py();
    let result = PyDict::new(py);
    for (index, model) in native_field(project, "models")?.try_iter()?.enumerate() {
        let model = model?;
        let sql = query_sql(&model)?;
        if mentions(&sql.extract::<String>()?) {
            result.set_item(
                index,
                render_test_cursor_intrinsics(sql, &model, adapter, None)?,
            )?;
        }
    }
    Ok(result)
}

pub(crate) fn overrides<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = project.py();
    let models = PyDict::new(py);
    for model in native_field(project, "models")?.try_iter()? {
        let model = model?;
        models.set_item(name_object(&model)?, model)?;
    }
    let tests: Vec<Bound<'py, PyAny>> = tests.extract()?;
    let mut indexes: Vec<usize> = Vec::new();
    let mut windowed: Vec<Bound<'py, PyAny>> = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        if model_payload(test)? && declares_cursor_window(test)? {
            indexes.push(index);
            windowed.push(test.clone());
        }
    }
    let chains = if indexes.is_empty() {
        Vec::new()
    } else {
        crate::sql_test_glue::_helpers::plan::plan_driver::chains(
            project,
            &PyTuple::new(py, windowed)?.into_any(),
        )?
        .extract::<Vec<Bound<'py, PyAny>>>()?
    };
    if indexes.len() != chains.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "zip() argument lengths differ",
        ));
    }
    let chains: std::collections::BTreeMap<_, _> = indexes.into_iter().zip(chains).collect();
    let result = PyDict::new(py);
    for (index, test) in tests.iter().enumerate() {
        if !model_payload(test)? {
            continue;
        }
        let overrides = native_field(&native_field(test, "payload")?, "model_query_overrides")?
            .cast_into::<PyDict>()?;
        let chain = chains.get(&index);
        if chain.is_none()
            && !overrides
                .values()
                .try_iter()?
                .map(|value| Ok(mentions(&value?.extract::<String>()?)))
                .collect::<PyResult<Vec<bool>>>()?
                .contains(&true)
        {
            continue;
        }
        let chain_names: Vec<String> = chain
            .map(|value| value.extract())
            .transpose()?
            .unwrap_or_default();
        let rendered = PyDict::new(py);
        for (name, sql) in overrides.iter() {
            let name: String = name.extract()?;
            let sql = if let Some(model) = models.get_item(&name)? {
                render_test_cursor_intrinsics(
                    sql,
                    &model,
                    adapter,
                    chain_names.contains(&name).then_some(test),
                )?
            } else {
                sql
            };
            rendered.set_item(name, sql)?;
        }
        if chain.is_some() {
            let mut chain_models: Vec<Bound<'py, PyAny>> = Vec::new();
            for name in chain_names {
                if let Some(model) = models.get_item(name)? {
                    chain_models.push(model);
                }
            }
            for model in declared_window_cursor_models(test, chain_models)? {
                let name = name_object(&model)?;
                if !rendered.contains(&name)? {
                    rendered.set_item(
                        name,
                        render_test_cursor_intrinsics(
                            query_sql(&model)?,
                            &model,
                            adapter,
                            Some(test),
                        )?,
                    )?;
                }
            }
        }
        result.set_item(index, rendered)?;
    }
    Ok(result)
}

pub(crate) fn functions<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = project.py();
    let mut result: Vec<Bound<'py, PyTuple>> = Vec::new();
    for function in native_field(project, "functions")?.try_iter()? {
        let function = function?;
        let target = native_field(&native_field(&function, "destination")?, "qualified_name")?;
        if target.is_none() {
            continue;
        }
        let mut fields = vec![native_field(&function, "name")?];
        for method in ["render_udf_call", "render_table_function_call"] {
            let rendered: String = call_with(
                &adapter.getattr(method)?,
                &[
                    ("target", target.clone()),
                    ("call_suffix_sql", string(py, SENTINEL)),
                ],
            )?
            .extract()?;
            let Some((prefix, suffix)) = rendered
                .split_once(SENTINEL)
                .filter(|(_, suffix)| !suffix.contains(SENTINEL))
            else {
                return Err(PyErr::from_value(
                    error_class(py, ErrorId::NativeSqlTestPlanning)?
                        .call1(("adapter SQL function call template is not deterministic",))?,
                ));
            };
            fields.push(string(py, prefix));
            fields.push(string(py, suffix));
        }
        result.push(PyTuple::new(py, fields)?);
    }
    PyTuple::new(py, result)
}
