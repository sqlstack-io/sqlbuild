//! Register the records functions and classes.

use crate::compiled_views::_helpers::project::records::compiled_project_record;
use crate::compiled_views::main::project::compile_inputs_view::compile_inputs_view;
use crate::compiled_views::main::project::compiled_audit_record::compiled_audit_record;
use crate::compiled_views::main::project::compiled_function_record::compiled_function_record;
use crate::compiled_views::main::project::compiled_model_record::compiled_model_record;
use crate::compiled_views::main::project::compiled_project_facts::compiled_project_facts;
use crate::compiled_views::main::project::compiled_seed_record::compiled_seed_record;
use crate::compiled_views::main::project::compiled_source_record::compiled_source_record;
use crate::compiled_views::main::project::declaration_rows::declaration_rows;
use crate::compiled_views::main::project::inputs_with::inputs_with;
use crate::compiled_views::main::project::plain_typed_value::plain_typed_value;
use crate::compiled_views::main::project::project_with::project_with;
use crate::compiled_views::main::project::record_model_analyses::record_model_analyses;
use crate::compiled_views::main::project::with_binding_diagnostics::with_binding_diagnostics;
use crate::compiled_views::main::project::with_type_recovery::with_type_recovery;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the records functions and classes.
pub fn register_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    for function in [
        wrap_pyfunction!(compiled_model_record, module)?,
        wrap_pyfunction!(compiled_audit_record, module)?,
        wrap_pyfunction!(compiled_function_record, module)?,
        wrap_pyfunction!(compiled_source_record, module)?,
        wrap_pyfunction!(compiled_seed_record, module)?,
        wrap_pyfunction!(compiled_project_record, module)?,
        wrap_pyfunction!(with_binding_diagnostics, module)?,
        wrap_pyfunction!(with_type_recovery, module)?,
        wrap_pyfunction!(project_with, module)?,
        wrap_pyfunction!(inputs_with, module)?,
        wrap_pyfunction!(compile_inputs_view, module)?,
        wrap_pyfunction!(plain_typed_value, module)?,
        wrap_pyfunction!(declaration_rows, module)?,
        wrap_pyfunction!(record_model_analyses, module)?,
        wrap_pyfunction!(compiled_project_facts, module)?,
    ] {
        module.add_function(function)?;
    }
    Ok(())
}
