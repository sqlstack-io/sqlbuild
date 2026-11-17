//! The dbt event stream: one output line in, the progress and row it adds out.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyDict;
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::dbt_events::main::consume_dbt_event::consume_dbt_event;
use sqlbuild_core::dbt_events::main::render_dbt_node_row::render_dbt_node_row;
use sqlbuild_core::dbt_events::models::{DbtRowPosition, DbtStreamState, DbtStreamUpdate};

use crate::bindings::_helpers::dbt_events::event::dbt_event;
use crate::bindings::_helpers::dbt_events::results::{native_result, python_result};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const STREAM_OPTIONS: (&[&str], &[&str]) =
    (&["display_total", "detail_by_unique_id", "use_color"], &[]);
const RENDER_OPTIONS: (&[&str], &[&str]) = (
    &["result", "use_color"],
    &["display_index", "display_total", "detail"],
);

/// What one dbt output line adds: a progress message, a node row, or both.
#[pyclass(module = "sqlbuild._native", frozen, get_all)]
pub(crate) struct DbtEventStreamUpdate {
    /// The node-started progress message.
    pub(crate) status_message: Option<String>,
    /// The started or finished node.
    pub(crate) result: Option<Py<PyAny>>,
    /// The rendered row and its messages.
    pub(crate) row: Option<String>,
    /// Whether `result` is a final result rather than a start.
    pub(crate) finished: bool,
}

/// Read dbt's JSON log lines into node progress, rows and results.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DbtEventStream {
    state: Mutex<DbtStreamState>,
    display_total: Option<i64>,
    detail_by_unique_id: Py<PyAny>,
    style: CliStyle,
}

impl DbtEventStream {
    fn detail(&self, py: Python<'_>, unique_id: &str) -> PyResult<String> {
        let details: &Bound<'_, PyAny> = self.detail_by_unique_id.bind(py);
        if details.is_none() {
            return Ok(String::new());
        }
        mapping_get_or(details, unique_id, "")?.extract()
    }

    fn update(&self, py: Python<'_>, update: DbtStreamUpdate) -> PyResult<DbtEventStreamUpdate> {
        let Some(row) = update.row else {
            return Ok(DbtEventStreamUpdate {
                status_message: update.status_message,
                result: None,
                row: None,
                finished: false,
            });
        };
        let position: DbtRowPosition = DbtRowPosition {
            display_index: Some(row.display_index),
            display_total: self.display_total,
        };
        let detail: String = self.detail(py, &row.result.unique_id)?;
        Ok(DbtEventStreamUpdate {
            status_message: update.status_message,
            result: Some(python_result(py, &row.result)?.unbind()),
            row: Some(render_dbt_node_row(
                &row.result,
                position,
                &detail,
                self.style,
            )),
            finished: row.finished,
        })
    }
}

#[pymethods]
impl DbtEventStream {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments: KeywordArguments<'_> =
            KeywordArguments::read(py, "DbtEventStream", options, STREAM_OPTIONS)?;
        Ok(Self {
            state: Mutex::new(DbtStreamState::default()),
            display_total: arguments.value("display_total")?.extract()?,
            detail_by_unique_id: arguments.value("detail_by_unique_id")?.unbind(),
            style: CliStyle {
                use_color: arguments.flag("use_color")?,
            },
        })
    }

    /// The update one output line makes, or `None` when it changes nothing.
    fn consume(&self, py: Python<'_>, line: &str) -> PyResult<Option<DbtEventStreamUpdate>> {
        let Some(event) = dbt_event(line)? else {
            return Ok(None);
        };
        let update: Option<DbtStreamUpdate> = {
            let mut state = self.state.lock().map_err(|_| {
                PyErr::new::<PyRuntimeError, _>("dbt event stream state is poisoned")
            })?;
            let (next, update) = consume_dbt_event(std::mem::take(&mut *state), &event);
            *state = next;
            update
        };
        update.map(|update| self.update(py, update)).transpose()
    }
}

/// One dbt node row and its messages.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_dbt_node_result(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "render_dbt_node_result", options, RENDER_OPTIONS)?;
    let position: DbtRowPosition = DbtRowPosition {
        display_index: arguments.value("display_index")?.extract()?,
        display_total: arguments.value("display_total")?.extract()?,
    };
    let detail: String = arguments
        .value("detail")?
        .extract::<Option<String>>()?
        .unwrap_or_default();
    Ok(render_dbt_node_row(
        &native_result(&arguments.value("result")?)?,
        position,
        &detail,
        CliStyle {
            use_color: arguments.flag("use_color")?,
        },
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DbtEventStream>()?;
    module.add_class::<DbtEventStreamUpdate>()?;
    module.add_function(wrap_pyfunction!(render_dbt_node_result, module)?)?;
    Ok(())
}
