//! Register the declaration records functions and classes.

use crate::discovery::main::records::discover_adapter_file::discover_adapter_file;
use crate::discovery::main::records::discover_audit_files::discover_audit_files;
use crate::discovery::main::records::discover_constant_files::discover_constant_files;
use crate::discovery::main::records::discover_enum_files::discover_enum_files;
use crate::discovery::main::records::discover_macro_files::discover_macro_files;
use crate::discovery::main::records::discover_model_schema_files::discover_model_schema_files;
use crate::discovery::main::records::discover_seed_files::discover_seed_files;
use crate::discovery::main::records::discover_sql_function_files::discover_sql_function_files;
use crate::discovery::main::records::discover_sql_hook_files::discover_sql_hook_files;
use crate::discovery::main::records::retained_discovery_session::retained_discovery_session;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the declaration records functions and classes.
pub fn register_declaration_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_enum_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_constant_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_model_schema_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_sql_function_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_sql_hook_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_audit_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_macro_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_seed_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_adapter_file, module)?)?;
    module.add_function(wrap_pyfunction!(retained_discovery_session, module)?)?;
    Ok(())
}
