//! Execution headers, SQL expectation rows and verbose inspection lines of CLI progress.

use pyo3::pyfunction;
use sqlbuild_core::progress::main::format_expectation_detail::format_expectation_detail as format_detail;
use sqlbuild_core::progress::main::format_expectation_name::format_expectation_name as format_name;
use std::sync::{Mutex, MutexGuard};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::progress::main::format_inspection_record::format_inspection_record;
use sqlbuild_core::progress::models::{ExpectationStep, InspectionRecord};
use sqlbuild_core::progress::types::DifferenceSample;
use sqlbuild_host::entries::models::AdapterHost;

use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::build_progress::main::progress::write_persistent::write_persistent;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const HEADER_OPTIONS: (&[&str], &[&str]) = (
    &["stream", "command", "target", "concurrency", "use_color"],
    &["engines"],
);

fn difference_samples(samples: &Bound<'_, PyAny>) -> PyResult<Vec<DifferenceSample>> {
    let mut rows: Vec<DifferenceSample> = Vec::new();
    for sample in samples.try_iter()? {
        rows.push(native_field(&sample?, "values")?.extract()?);
    }
    Ok(rows)
}

pub(crate) fn expectation_step(step: &Bound<'_, PyAny>) -> PyResult<ExpectationStep> {
    let mut column_differences: Vec<(String, String, String)> = Vec::new();
    for difference in native_field(step, "column_differences")?.try_iter()? {
        let difference: Bound<'_, PyAny> = difference?;
        column_differences.push((
            native_field(&difference, "name")?.str()?.to_string(),
            native_field(&difference, "actual")?.str()?.to_string(),
            native_field(&difference, "expected")?.str()?.to_string(),
        ));
    }
    Ok(ExpectationStep {
        model_name: native_field(step, "model_name")?.extract()?,
        outcome: native_field(step, "outcome")?.str()?.to_string(),
        actual_row_count: native_field(step, "actual_row_count")?.extract()?,
        expected_row_count: native_field(step, "expected_row_count")?.extract()?,
        mismatched_row_count: native_field(step, "mismatched_row_count")?.extract()?,
        unexpected_row_count: native_field(step, "unexpected_row_count")?.extract()?,
        missing_row_count: native_field(step, "missing_row_count")?.extract()?,
        unexpected_samples: difference_samples(&native_field(step, "unexpected_samples")?)?,
        missing_samples: difference_samples(&native_field(step, "missing_samples")?)?,
        column_differences,
    })
}

fn inspection_record(record: &Bound<'_, PyAny>) -> PyResult<InspectionRecord> {
    Ok(InspectionRecord {
        sql: native_field(record, "sql")?.extract()?,
        elapsed_seconds: native_field(record, "elapsed_seconds")?.extract()?,
        row_count: native_field(record, "row_count")?.extract()?,
        error: native_field(record, "error")?.extract()?,
    })
}

/// Write each inspection read as one truncated stderr line and keep a running total.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct VerboseInspectionWriter {
    totals: Mutex<(usize, f64)>,
}

impl VerboseInspectionWriter {
    fn locked(&self) -> PyResult<MutexGuard<'_, (usize, f64)>> {
        self.totals
            .lock()
            .map_err(|_| PyRuntimeError::new_err("inspection writer lock is poisoned"))
    }
}

fn write_stderr_line(py: Python<'_>, text: &str) -> PyResult<()> {
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    write_persistent(py, &stderr, &format!("{text}\n"))
}

#[pymethods]
impl VerboseInspectionWriter {
    #[new]
    fn new() -> Self {
        Self {
            totals: Mutex::new((0, 0.0)),
        }
    }

    /// Write one inspection read; safe to call from inspection worker threads.
    fn write(&self, py: Python<'_>, record: &Bound<'_, PyAny>) -> PyResult<()> {
        let record: InspectionRecord = inspection_record(record)?;
        {
            let mut totals: MutexGuard<'_, (usize, f64)> = self.locked()?;
            totals.0 += 1;
            totals.1 += record.elapsed_seconds;
        }
        write_stderr_line(py, &format_inspection_record(&record))
    }

    /// Write the read count and cumulative query time when anything was read.
    fn write_total(&self, py: Python<'_>) -> PyResult<()> {
        let (count, seconds) = *self.locked()?;
        if count == 0 {
            return Ok(());
        }
        write_stderr_line(
            py,
            &format!("Inspection queries: {count} ({seconds:.2}s cumulative query time)"),
        )
    }

    /// Render one inspection read as a single truncated diagnostic line.
    #[staticmethod]
    #[pyo3(signature = (*, record))]
    fn format_record(record: &Bound<'_, PyAny>) -> PyResult<String> {
        Ok(format_inspection_record(&inspection_record(record)?))
    }
}

/// Run `body`, writing each inspection read and then a total to stderr while `enabled`.
pub(crate) fn with_verbose_inspection<'py, T>(
    py: Python<'py>,
    enabled: bool,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    if !enabled {
        return body();
    }
    let writer: Bound<'py, VerboseInspectionWriter> =
        Bound::new(py, VerboseInspectionWriter::new())?;
    let sink: Bound<'py, PyAny> = host_entry(py, AdapterHost::InspectionQuerySink)?
        .call1((native_field(&writer, "write")?,))?;
    let value: T = within(&sink, |_| body())?;
    writer.get().write_total(py)?;
    Ok(value)
}

/// Format failing row count detail for an expectation row.
#[pyfunction]
pub(crate) fn format_expectation_detail(step_result: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format_detail(&expectation_step(step_result)?))
}

/// Format a comparison result name as a user-facing expectation name.
#[pyfunction]
pub(crate) fn format_expectation_name(model_name: &str) -> String {
    format_name(model_name)
}
