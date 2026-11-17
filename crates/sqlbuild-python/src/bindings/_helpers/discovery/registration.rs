//! Register the native project discovery functions and classes.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::discovery::main::failures::register_guidance::register_guidance;
use sqlbuild_python_front::discovery::main::failures::register_raised::register_raised;
use sqlbuild_python_front::discovery::main::headers::register_header_declarations::register_header_declarations;
use sqlbuild_python_front::discovery::main::headers::register_header_values::register_header_values;
use sqlbuild_python_front::discovery::main::headers::register_hook_entries::register_hook_entries;
use sqlbuild_python_front::discovery::main::headers::register_key_checks::register_key_checks;
use sqlbuild_python_front::discovery::main::project::register_locations::register_locations;
use sqlbuild_python_front::discovery::main::project::register_project_files::register_project_files;
use sqlbuild_python_front::discovery::main::project::register_project_validation::register_project_validation;
use sqlbuild_python_front::discovery::main::python_files::register_imported_python_nodes::register_imported_python_nodes;
use sqlbuild_python_front::discovery::main::python_files::register_python_function_files::register_python_function_files;
use sqlbuild_python_front::discovery::main::python_files::register_python_hook_files::register_python_hook_files;
use sqlbuild_python_front::discovery::main::python_files::register_python_module_files::register_python_module_files;
use sqlbuild_python_front::discovery::main::records::register_declaration_records::register_declaration_records;
use sqlbuild_python_front::discovery::main::records::register_description_inputs::register_description_inputs;
use sqlbuild_python_front::discovery::main::records::register_model_records::register_model_records;
use sqlbuild_python_front::discovery::main::records::register_sql_test_records::register_sql_test_records;
use sqlbuild_python_front::discovery::main::records::register_yaml_records::register_yaml_records;
use sqlbuild_python_front::discovery::main::snapshots::register_scope_snapshot::register_scope_snapshot;
use sqlbuild_python_front::discovery::main::snapshots::register_snapshot::register_snapshot;
use sqlbuild_python_front::discovery::main::sources::register_declaration_files::register_declaration_files;
use sqlbuild_python_front::discovery::main::sources::register_yaml_entries::register_yaml_entries;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_project_files(module)?;
    register_snapshot(module)?;
    register_scope_snapshot(module)?;
    register_declaration_files(module)?;
    register_declaration_records(module)?;
    register_python_function_files(module)?;
    register_python_hook_files(module)?;
    register_python_module_files(module)?;
    register_model_records(module)?;
    register_sql_test_records(module)?;
    register_yaml_records(module)?;
    register_raised(module)?;
    register_locations(module)?;
    register_header_declarations(module)?;
    register_key_checks(module)?;
    register_project_validation(module)?;
    register_yaml_entries(module)?;
    register_header_values(module)?;
    register_guidance(module)?;
    register_description_inputs(module)?;
    register_hook_entries(module)?;
    register_imported_python_nodes(module)?;
    Ok(())
}
