//! Relations named in SQL and temporary tables it creates, for hard-coded name checks.

use crate::analysis::types::RelationRow;
use pyo3::PyErr;
use sqlbuild_analysis::relation_names::errors::RelationNamesFailure;
use sqlbuild_analysis::relation_names::models::RelationName;
use sqlbuild_core::panics::main::native_failure::native_failure;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;

const RELATION_NAMES_CONTEXT: &str = "native relation name extraction";

pub(crate) fn relation_row(relation: RelationName) -> RelationRow {
    (relation.name, relation.schema, relation.database)
}

/// An unknown dialect as the wheel's `ValueError`; anything else as an internal failure.
pub(crate) fn relation_names_error(failure: RelationNamesFailure) -> PyErr {
    match failure {
        RelationNamesFailure::UnknownDialect(message) => compiler_error(message),
        RelationNamesFailure::Internal(reason) => {
            compiler_error(native_failure(RELATION_NAMES_CONTEXT, &reason))
        }
    }
}
