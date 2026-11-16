//! How often each compile stage ran in this process, for tests that assert stage work.

use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyModuleMethods;
use pyo3::wrap_pyfunction;
use std::collections::BTreeMap;

use pyo3::prelude::{Py, PyRefMut, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::{pyclass, pyfunction};

/// Compile stage runs since the last reset, by stage.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct StageWork {
    runs: BTreeMap<&'static str, u64>,
}

static STAGE_WORK: PyOnceLock<Py<StageWork>> = PyOnceLock::new();

fn stage_work(py: Python<'_>) -> PyResult<PyRefMut<'_, StageWork>> {
    Ok(STAGE_WORK
        .get_or_try_init(py, || Py::new(py, StageWork::default()))?
        .bind(py)
        .borrow_mut())
}

/// Add `count` runs of `stage`.
pub(crate) fn count_stage(py: Python<'_>, stage: &'static str, count: u64) -> PyResult<()> {
    *stage_work(py)?.runs.entry(stage).or_default() += count;
    Ok(())
}

/// Report the runs of each compile stage so far, optionally resetting them.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
pub(crate) fn compile_stage_work_info(
    py: Python<'_>,
    reset: bool,
) -> PyResult<BTreeMap<String, u64>> {
    let mut work = stage_work(py)?;
    let info: BTreeMap<String, u64> = work
        .runs
        .iter()
        .map(|(stage, count)| ((*stage).to_owned(), *count))
        .collect();
    if reset {
        *work = StageWork::default();
    }
    Ok(info)
}

/// Register `compile_stage_work_info` on the native module.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_stage_work_info, module)?)
}
