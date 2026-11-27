//! A `@loader` run as SQLLoad's Python host: its table crossed by `sqlload-python-host`, transforms through `sqlload_api`.

use std::path::{Path, PathBuf};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use pyo3::types::PyAnyMethods;
use pyo3::{PyResult, Python};
use sqlload::python_sources::models::{PythonSource, TableRead, TransformCall};
use sqlload::python_sources::types::{BatchStream, PythonHost};
use sqlload_python_host::host::constants::API_HOST_MODULE;
use sqlload_python_host::host::main::error_text::error_text;
use sqlload_python_host::host::main::table_stream::table_stream;
use sqlload_python_host::host::main::transform_stream::transform_stream;

use crate::runs::constants::RENAME_PROBLEM;
use crate::runs::models::LoaderHost;

impl PythonHost for LoaderHost {
    fn discover(
        &self,
        _root: &Path,
        _python_paths: &[PathBuf],
    ) -> Result<Vec<PythonSource>, String> {
        Ok(Vec::new())
    }

    fn read_table(&self, request: &TableRead) -> Result<Box<dyn BatchStream>, String> {
        let text = serde_json::to_string(request).map_err(|error| error.to_string())?;
        Python::attach(|py| -> PyResult<Result<Box<dyn BatchStream>, String>> {
            let run = self.run.bind(py);
            let items = run.call_method1("table_items", (text,))?;
            let snapshot = match request.logged {
                true => Some(run.getattr("current_state")?),
                false => None,
            };
            Ok(table_stream(&items, snapshot.as_ref(), request))
        })
        .map_err(|error| error_text(&error))?
    }

    fn table_state(&self, _table: &str) -> Result<Option<String>, String> {
        Python::attach(|py| -> PyResult<Option<String>> {
            self.run.bind(py).call_method0("final_state")?.extract()
        })
        .map_err(|error| error_text(&error))
    }

    fn transform_version(&self, name: &str) -> Result<String, String> {
        let checked = Python::attach(|py| -> PyResult<(Option<String>, Option<String>)> {
            py.import(API_HOST_MODULE)?
                .call_method1("transform_version", (name,))?
                .extract()
        })
        .map_err(|error| error_text(&error))?;
        match checked {
            (Some(version), _) => Ok(version),
            (None, problem) => Err(problem.unwrap_or_else(|| format!("{name} cannot run"))),
        }
    }

    fn transform(
        &self,
        call: &TransformCall,
        rows: Vec<RecordBatch>,
        schema: SchemaRef,
    ) -> Result<Box<dyn BatchStream>, String> {
        transform_stream(call, rows, schema)
    }

    fn rename(
        &self,
        _name: &str,
        _columns: Option<&str>,
        _context: &str,
    ) -> Result<String, String> {
        Err(RENAME_PROBLEM.to_owned())
    }

    fn finish(&self) {}
}
