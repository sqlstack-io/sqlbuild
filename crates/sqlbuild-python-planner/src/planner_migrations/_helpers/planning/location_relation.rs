//! The migration relation that names one compiled relation location, built natively.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The `MigrationRelation` that names `location`, as `migration_relation_for_location` builds it.
pub(crate) fn location_relation<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        location.py(),
        RecordId::MigrationRelation,
        &[
            ("database", native_field(location, "database")?),
            ("schema", native_field(location, "schema")?),
            ("name", native_field(location, "name")?),
        ],
    )
}
