//! Register the native model analysis session functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use crate::bindings::_helpers::analysis_session::oracle;
use crate::bindings::_helpers::analysis_session::outcome_objects;
use sqlbuild_python_project::analysis_session::main::register_fixture_columns::register_fixture_columns;
use sqlbuild_python_project::analysis_session::main::register_query_columns::register_query_columns;
use sqlbuild_python_project::analysis_session::main::register_session::register_session;
use sqlbuild_python_project::analysis_session::main::register_stage_functions::register_stage_functions;
use sqlbuild_python_project::analysis_session::models::FixtureColumnInferences;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_session(module)?;
    register_fixture_columns(module)?;
    module.add_class::<FixtureColumnInferences>()?;
    outcome_objects::register(module)?;
    register_query_columns(module)?;
    register_stage_functions(module)?;
    oracle::register(module)
}
