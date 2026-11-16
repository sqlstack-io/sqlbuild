//! Streamed clone item lines and fingerprint propagation progress, as executor callbacks.

use std::collections::HashMap;

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::clone_text::main::render_clone_fingerprint_interrupted_line::render_clone_fingerprint_interrupted_line;
use sqlbuild_core::clone_text::main::render_clone_fingerprint_progress_line::render_clone_fingerprint_progress_line;
use sqlbuild_core::clone_text::main::render_clone_item_line::render_clone_item_line;
use sqlbuild_core::clone_text::models::{CloneItemText, CloneStatusKind, FingerprintProgress};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

/// One executor `CloneItemResult` as the command renders it.
pub(crate) fn clone_item_text(item: &Bound<'_, PyAny>) -> PyResult<CloneItemText> {
    let duration: Bound<'_, PyAny> = native_field(item, "duration_seconds")?;
    Ok(CloneItemText {
        name: native_field(item, "name")?.extract()?,
        action: native_field(item, "action")?.getattr("value")?.extract()?,
        status: CloneStatusKind::from_value(
            &native_field(item, "status")?
                .getattr("value")?
                .extract::<String>()?,
        ),
        message: optional_text(native_field(item, "message")?)?,
        origin_relation: optional_text(native_field(item, "origin_relation")?)?,
        destination_relation: optional_text(native_field(item, "destination_relation")?)?,
        duration_seconds: if duration.is_none() {
            None
        } else {
            Some(duration.extract()?)
        },
    })
}

/// Write `line` and a newline to `stream` and flush.
pub(crate) fn write_line(stream: &Bound<'_, PyAny>, line: &str) -> PyResult<()> {
    stream.call_method1("write", (format!("{line}\n"),))?;
    stream.call_method0("flush")?;
    Ok(())
}

/// Print each finished clone item and record it in the event output.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CloneItemPrinter {
    pub(crate) stream: Py<PyAny>,
    pub(crate) style: CliStyle,
    pub(crate) relation_width: usize,
    pub(crate) event_writer: Py<PyAny>,
    pub(crate) resource_types: HashMap<String, String>,
}

#[pymethods]
impl CloneItemPrinter {
    #[pyo3(signature = (*, index, total, item))]
    fn __call__(
        &self,
        py: Python<'_>,
        index: usize,
        total: usize,
        item: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let text: CloneItemText = clone_item_text(item)?;
        write_line(
            self.stream.bind(py),
            &render_clone_item_line((index, total), &text, self.relation_width, self.style),
        )?;
        let resource_type: &String = self
            .resource_types
            .get(&text.name)
            .ok_or_else(|| PyKeyError::new_err(text.name.clone()))?;
        let options: Bound<'_, PyDict> = PyDict::new(py);
        pyo3::types::PyDictMethods::set_item(&options, "item", item)?;
        pyo3::types::PyDictMethods::set_item(&options, "resource_type", resource_type)?;
        self.event_writer
            .bind(py)
            .call_method("write_clone_result", (), Some(&options))?;
        Ok(())
    }
}

/// Report confirmed fingerprint batches and, on interruption, what was not confirmed.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct CloneFingerprintReporter {
    pub(crate) stream: Py<PyAny>,
    pub(crate) style: CliStyle,
    pub(crate) progress: FingerprintProgress,
}

#[pymethods]
impl CloneFingerprintReporter {
    #[pyo3(signature = (*, completed, total, pending_identities))]
    fn __call__(
        &mut self,
        py: Python<'_>,
        completed: i64,
        total: i64,
        pending_identities: Vec<String>,
    ) -> PyResult<()> {
        self.progress = FingerprintProgress {
            completed,
            total,
            pending_identities,
        };
        if completed == 0 {
            return Ok(());
        }
        write_line(
            self.stream.bind(py),
            &render_clone_fingerprint_progress_line(completed, total, self.style),
        )
    }

    pub(crate) fn write_interrupted(&self, py: Python<'_>) -> PyResult<()> {
        match render_clone_fingerprint_interrupted_line(&self.progress, self.style) {
            Some(line) => write_line(self.stream.bind(py), &line),
            None => Ok(()),
        }
    }
}
