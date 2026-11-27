//! Resolve authored cursor input fields into explicit effective roles.

use crate::compile_attachment::_helpers::cursor_roles::CURSOR_INPUTS_KEY;
use crate::compile_attachment::_helpers::cursor_roles::ROLLING_WINDOW_STRATEGY;
use crate::compile_attachment::_helpers::cursor_roles::WATERMARK_STRATEGY;
use crate::compile_attachment::_helpers::cursor_roles::role_blocks;
use crate::compile_attachment::_helpers::cursor_roles::string_map;
use crate::compiled_views::main::model_inputs::input_reference_rows::input_reference_rows;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// Resolve authored cursor input fields into explicit effective roles.
#[pyfunction]
#[pyo3(signature = (*, model))]
pub fn resolve_cursor_input_roles<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let values: Bound<'py, PyAny> = native_field(model, "config")?.getattr("values")?;
    let authored: Bound<'py, PyAny> = mapping_get(&values, CURSOR_INPUTS_KEY)?;
    let strategy: Bound<'py, PyAny> = mapping_get(&values, "microbatch_strategy")?;
    let rolling: bool = strategy.eq(ROLLING_WINDOW_STRATEGY)?;
    let mut filter: Bound<'py, PyDict> = PyDict::new(py);
    let mut watermark: Bound<'py, PyDict> = PyDict::new(py);
    match authored.cast::<PyDict>() {
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
