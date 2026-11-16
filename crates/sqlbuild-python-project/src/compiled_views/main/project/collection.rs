//! The project's resource collection named `field`; any other field is read by attribute.

use crate::compiled_views::_helpers::project::project_reads::AUDITS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::FUNCTIONS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::MODELS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::SEEDS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::SOURCES_FIELD;
use crate::compiled_views::main::project::audits::audits;
use crate::compiled_views::main::project::compiled_models::compiled_models;
use crate::compiled_views::main::project::functions::functions;
use crate::compiled_views::main::project::seeds::seeds;
use crate::compiled_views::main::project::sources::sources;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's resource collection named `field`; any other field is read by attribute.
pub fn collection<'py>(project: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    match field {
        MODELS_FIELD => compiled_models(project),
        SOURCES_FIELD => sources(project),
        SEEDS_FIELD => seeds(project),
        FUNCTIONS_FIELD => functions(project),
        AUDITS_FIELD => audits(project),
        _ => native_field(project, field),
    }
}
