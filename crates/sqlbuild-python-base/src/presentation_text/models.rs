//! Public records of presentation text.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::pyclass;
use pyo3::pymethods;
use sqlbuild_core::cli_text::models::CliStyle as NativeStyle;

/// Semantic CLI styling facade used by human-output formatters.
#[pyclass(name = "CliStyle", module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct PythonCliStyle {
    #[pyo3(get)]
    pub use_color: bool,
}

impl PythonCliStyle {
    pub(crate) fn native(&self) -> NativeStyle {
        NativeStyle {
            use_color: self.use_color,
        }
    }
}

#[pymethods]
impl PythonCliStyle {
    #[new]
    #[pyo3(signature = (*, use_color))]
    pub fn new(use_color: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            use_color: use_color.is_truthy()?,
        })
    }

    pub(crate) fn title(&self, text: &str) -> String {
        self.native().title(text)
    }

    pub(crate) fn section(&self, text: &str) -> String {
        self.native().section(text)
    }

    pub(crate) fn label(&self, text: &str) -> String {
        self.native().label(text)
    }

    pub(crate) fn value(&self, text: &str) -> String {
        self.native().value(text)
    }

    pub(crate) fn accent(&self, text: &str) -> String {
        self.native().accent(text)
    }

    pub(crate) fn accent_strong(&self, text: &str) -> String {
        self.native().accent_strong(text)
    }

    pub(crate) fn plan_section(&self, text: &str) -> String {
        self.native().plan_section(text)
    }

    pub(crate) fn object_name(&self, text: &str) -> String {
        self.native().object_name(text)
    }

    pub(crate) fn command(&self, text: &str) -> String {
        self.native().command(text)
    }

    pub(crate) fn muted(&self, text: &str) -> String {
        self.native().muted(text)
    }

    pub(crate) fn success(&self, text: &str) -> String {
        self.native().success(text)
    }

    pub(crate) fn success_strong(&self, text: &str) -> String {
        self.native().success_strong(text)
    }

    pub(crate) fn warning(&self, text: &str) -> String {
        self.native().warning(text)
    }

    pub(crate) fn warning_strong(&self, text: &str) -> String {
        self.native().warning_strong(text)
    }

    pub(crate) fn error(&self, text: &str) -> String {
        self.native().error(text)
    }

    pub(crate) fn error_strong(&self, text: &str) -> String {
        self.native().error_strong(text)
    }

    pub(crate) fn error_muted(&self, text: &str) -> String {
        self.native().error_muted(text)
    }

    pub(crate) fn log_label(&self, text: &str) -> String {
        self.native().log_label(text)
    }

    pub(crate) fn dbt_section(&self, text: &str) -> String {
        self.native().dbt_section(text)
    }

    pub(crate) fn dbt_label(&self, text: &str) -> String {
        self.native().dbt_label(text)
    }

    pub(crate) fn dbt_object_name(&self, text: &str) -> String {
        self.native().dbt_object_name(text)
    }

    pub(crate) fn dbt_execution_label(&self, text: &str) -> String {
        self.native().dbt_execution_label(text)
    }

    /// Style status text according to the status word.
    #[pyo3(signature = (*, status, text = None))]
    pub(crate) fn status(&self, status: &str, text: Option<&str>) -> String {
        self.native().status_text(status, text.unwrap_or(status))
    }
}
