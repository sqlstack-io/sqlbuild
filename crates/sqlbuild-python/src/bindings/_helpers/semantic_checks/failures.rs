//! The exception a failed native semantic completion step raises.

use pyo3::PyErr;
use pyo3::exceptions::{PyTypeError, PyValueError};
use sqlbuild_analysis::semantic_checks::models::SemanticFailure;
use sqlbuild_core::constants::PANIC_MESSAGE;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::type_system::normalization::normalization_error;

const SEMANTIC_CONTEXT: &str = "native semantic completion";
/// The wheel's `TypeError` for `tokenize(sql, dialect=None)`.
const NO_DIALECT_MESSAGE: &str = "argument 'dialect': 'None' is not an instance of 'str'";

/// The error Python's semantic completion raised for the same input, or `NativeCompilerError`.
pub(crate) fn semantic_error(failure: &SemanticFailure) -> PyErr {
    match failure {
        SemanticFailure::NoDialect => PyTypeError::new_err(NO_DIALECT_MESSAGE),
        SemanticFailure::UnknownDialect(name) => {
            PyValueError::new_err(format!("Unknown dialect: {name}"))
        }
        SemanticFailure::TypeNormalization(error) => normalization_error(error),
        SemanticFailure::Internal(reason) => internal_error(reason),
    }
}

/// An internal native failure as `NativeCompilerError`, keeping an already placed context.
pub(crate) fn internal_error(reason: &str) -> PyErr {
    if is_compiler_panic(reason) && reason != PANIC_MESSAGE {
        return compiler_error(reason);
    }
    compiler_error(native_failure(SEMANTIC_CONTEXT, reason))
}
