//! Cursor reads answered together in batched `UNION ALL` statements.

use crate::live_reads::_helpers::bounded::batch_task;
use crate::live_reads::_helpers::bounded::run_bounded;
use crate::planner_warehouse::models::CursorBatch;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyFloat, PyTuple};
use std::collections::HashMap;

/// Run `batches` at most `concurrency` at a time: each answered read's outcome, and failed batches' reads.
pub(crate) fn run_cursor_batches<'py>(
    py: Python<'py>,
    batches: &[CursorBatch],
    reader: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (concurrency, on_complete): (i64, Option<Bound<'py, PyAny>>),
) -> PyResult<(Vec<(usize, Bound<'py, PyAny>)>, Vec<usize>)> {
    let tasks: Vec<Bound<'py, PyAny>> = batches
        .iter()
        .map(|batch| batch_task(py, batch.sql.clone(), reader))
        .collect::<PyResult<_>>()?;
    let results = run_bounded(py, tasks, concurrency.max(1), on_complete)?;
    let mut answered: Vec<(usize, Bound<'py, PyAny>)> = Vec::new();
    let mut failed: Vec<usize> = Vec::new();
    for (batch, result) in batches.iter().zip(results) {
        let rows: Bound<'py, PyAny> = result.get_item(0)?;
        if !result.get_item(2)?.is_none() || rows.is_none() {
            failed.extend(batch.reads.iter().map(|(index, _)| *index));
            continue;
        }
        let elapsed: f64 = result.get_item(1)?.extract()?;
        let mut by_read: HashMap<usize, Bound<'py, PyAny>> = HashMap::new();
        for row in rows.try_iter()? {
            let row: Bound<'py, PyAny> = row?;
            let read: usize = read_position(&row.get_item(0)?)?;
            by_read.insert(read, row);
        }
        for (position, (index, has_min)) in batch.reads.iter().enumerate() {
            let row: Bound<'py, PyAny> = by_read
                .remove(&position)
                .unwrap_or_else(|| py.None().into_bound(py));
            let outcome = PyTuple::new(
                py,
                [
                    bound_row(&row, *has_min)?,
                    PyFloat::new(py, elapsed).into_any(),
                    py.None().into_bound(py),
                ],
            )?;
            answered.push((*index, outcome.into_any()));
        }
    }
    Ok((answered, failed))
}

/// A batch row's `_read` position, which a driver may return as an integer or a decimal.
fn read_position(value: &Bound<'_, PyAny>) -> PyResult<usize> {
    if let Ok(position) = value.extract::<usize>() {
        return Ok(position);
    }
    let text: String = value.str()?.extract()?;
    text.parse::<usize>().map_err(|_| {
        PyValueError::new_err(format!("cursor batch row has no read position: {text}"))
    })
}

/// A batch row `(_read, _min, _max)` as the row its read's own statement returns.
fn bound_row<'py>(row: &Bound<'py, PyAny>, has_min: bool) -> PyResult<Bound<'py, PyAny>> {
    let py = row.py();
    if row.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let bounds: Vec<Bound<'py, PyAny>> = if has_min {
        vec![row.get_item(1)?, row.get_item(2)?]
    } else {
        vec![row.get_item(2)?]
    };
    Ok(PyTuple::new(py, bounds)?.into_any())
}
