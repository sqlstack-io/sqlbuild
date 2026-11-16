//! Errors model configuration reports to Python.

use pyo3::pyclass;

/// One exact error: the Python class to raise, its message, code, help and named config key.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
#[derive(Debug)]
pub struct NativeConfigError {
    pub class_name: &'static str,
    pub message: String,
    pub code: Option<&'static str>,
    pub help: Option<String>,
    pub key: Option<String>,
}
