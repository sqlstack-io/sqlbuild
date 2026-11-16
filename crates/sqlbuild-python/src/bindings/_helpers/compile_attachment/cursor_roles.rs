//! A compiled model's authored cursor inputs resolved into filter and watermark roles.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString};

use crate::bindings::_helpers::compiled_views::model_input_view::input_reference_rows;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

const CURSOR_INPUTS_KEY: &str = "cursor_inputs";
const FILTER_ROLE: &str = "filter";
const WATERMARK_ROLE: &str = "watermark";
const WATERMARK_STRATEGY: &str = "watermark";
const ROLLING_WINDOW_STRATEGY: &str = "rolling_window";

/// Watermark-strategy cursor inputs: each relation's column under the roles it declares.
fn role_blocks<'py>(
    authored: &Bound<'py, PyDict>,
    (filter, watermark): (&Bound<'py, PyDict>, &Bound<'py, PyDict>),
) -> PyResult<()> {
    for (relation, block) in authored.iter() {
        let (true, Ok(block)) = (
            relation.is_instance_of::<PyString>(),
            block.downcast::<PyDict>(),
        ) else {
            continue;
        };
        let column: Option<Bound<'py, PyAny>> = block.get_item("column")?;
        let roles: Option<Bound<'py, PyAny>> = block.get_item("roles")?;
        let (Some(column), Some(roles)) = (column, roles) else {
            continue;
        };
        let (true, Ok(roles)) = (
            column.is_instance_of::<PyString>(),
            roles.downcast::<PyList>(),
        ) else {
            continue;
        };
        if roles.contains(FILTER_ROLE)? {
            filter.set_item(&relation, &column)?;
        }
        if roles.contains(WATERMARK_ROLE)? {
            watermark.set_item(&relation, &column)?;
        }
    }
    Ok(())
}

/// The string-to-string entries of an authored mapping.
fn string_map<'py>(authored: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(authored.py());
    for (key, item) in authored.iter() {
        if key.is_instance_of::<PyString>() && item.is_instance_of::<PyString>() {
            result.set_item(key, item)?;
        }
    }
    Ok(result)
}

/// Resolve authored cursor input fields into explicit effective roles.
#[pyfunction]
#[pyo3(signature = (*, model))]
pub(crate) fn resolve_cursor_input_roles<'py>(
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let values: Bound<'py, PyAny> = native_field(model, "config")?.getattr("values")?;
    let authored: Bound<'py, PyAny> = mapping_get(&values, CURSOR_INPUTS_KEY)?;
    let strategy: Bound<'py, PyAny> = mapping_get(&values, "microbatch_strategy")?;
    let rolling: bool = strategy.eq(ROLLING_WINDOW_STRATEGY)?;
    let mut filter: Bound<'py, PyDict> = PyDict::new(py);
    let mut watermark: Bound<'py, PyDict> = PyDict::new(py);
    match authored.downcast::<PyDict>() {
        Ok(authored) if strategy.eq(WATERMARK_STRATEGY)? => {
            role_blocks(authored, (&filter, &watermark))?;
        }
        Ok(authored) => {
            filter = string_map(authored)?;
            if !rolling {
                watermark = filter.clone();
            }
        }
        Err(_) => {
            let cursor: Bound<'py, PyAny> = mapping_get(&values, "cursor")?;
            if cursor.is_instance_of::<PyString>() {
                for ReferenceRow { name, .. } in input_reference_rows(model)? {
                    filter.set_item(name, &cursor)?;
                }
            }
            if !rolling {
                watermark = filter.clone();
            }
        }
    }
    new_record_with(
        py,
        RecordId::CursorInputRoles,
        &[
            ("filter_inputs", filter.into_any()),
            ("watermark_inputs", watermark.into_any()),
        ],
    )
}
