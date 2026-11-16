//! `CliDocument`: simple static CLI output built line by line.

use std::sync::{Mutex, MutexGuard};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliDocument as Document;

use crate::presentation_text::_helpers::functions::style_of;

/// Build simple static CLI output with semantic styling.
#[pyclass(module = "sqlbuild._native", name = "CliDocument", frozen)]
pub(crate) struct CliDocument {
    document: Mutex<Document>,
}

impl CliDocument {
    fn lines(&self) -> PyResult<MutexGuard<'_, Document>> {
        self.document
            .lock()
            .map_err(|_| PyRuntimeError::new_err("CLI document lock is poisoned"))
    }
}

#[pymethods]
impl CliDocument {
    #[new]
    fn new(style: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            document: Mutex::new(Document::new(style_of(style)?)),
        })
    }

    #[pyo3(signature = (*, text, suffix = None))]
    fn header(&self, text: &str, suffix: Option<&str>) -> PyResult<()> {
        self.lines()?.header(text, suffix);
        Ok(())
    }

    fn blank(&self) -> PyResult<()> {
        self.lines()?.lines.push(String::new());
        Ok(())
    }

    #[pyo3(signature = (*, label, value, value_padding = " "))]
    fn field(&self, label: &str, value: &str, value_padding: &str) -> PyResult<()> {
        self.lines()?.field(label, (value, value_padding));
        Ok(())
    }

    #[pyo3(signature = (*, rows, label_width = None))]
    fn fields(&self, rows: Vec<(String, String)>, label_width: Option<usize>) -> PyResult<()> {
        self.lines()?.fields(&rows, label_width);
        Ok(())
    }

    fn section(&self, text: &str) -> PyResult<()> {
        self.lines()?.section(text);
        Ok(())
    }

    fn title_section(&self, text: &str) -> PyResult<()> {
        self.lines()?.title_section(text);
        Ok(())
    }

    fn line(&self, text: &str) -> PyResult<()> {
        self.lines()?.lines.push(text.to_owned());
        Ok(())
    }

    fn items(&self, values: Vec<String>) -> PyResult<()> {
        self.lines()?.items(&values);
        Ok(())
    }

    #[pyo3(signature = (*, prefix, command, style_command = true))]
    fn command_line(&self, prefix: &str, command: &str, style_command: bool) -> PyResult<()> {
        self.lines()?.command_line(prefix, (command, style_command));
        Ok(())
    }

    #[pyo3(signature = (*, commands, numbered = false, style_command = true))]
    fn commands(&self, commands: Vec<String>, numbered: bool, style_command: bool) -> PyResult<()> {
        self.lines()?.commands(&commands, (numbered, style_command));
        Ok(())
    }

    #[pyo3(signature = (*, trailing_newline = true))]
    fn render(&self, trailing_newline: bool) -> PyResult<String> {
        Ok(self.lines()?.render(trailing_newline))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CliDocument>()
}
