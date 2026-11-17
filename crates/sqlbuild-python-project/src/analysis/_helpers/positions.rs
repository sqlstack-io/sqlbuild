//! Python methods of authored binding positions.

use crate::analysis::_helpers::native_classes::PositionInput;
use crate::analysis::models::BindingPositions;
use pyo3::exceptions::PyValueError;
use pyo3::{PyResult, pymethods};
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

#[pymethods]
impl BindingPositions {
    #[new]
    pub(crate) fn new(request: PositionInput) -> PyResult<Self> {
        compiler_guard(|| {
            validation::BindingPositions::new(request.into())
                .map(|inner| Self { inner })
                .map_err(PyValueError::new_err)
        })
    }

    fn position(
        &self,
        start: Option<usize>,
        line: Option<usize>,
        column: Option<usize>,
        message: &str,
    ) -> PyResult<(Option<usize>, Option<usize>)> {
        compiler_guard(|| {
            self.inner
                .position(start, line, column, message)
                .map_err(PyValueError::new_err)
        })
    }
}
