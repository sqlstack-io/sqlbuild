//! The first row `cursor.fetchone()` returns, or `None`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::warehouse_access::models::SqliteCursor;

/// The first row `cursor.fetchone()` returns, or `None`.
pub fn fetch_one<'py>(cursor: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    if let Ok(cursor) = cursor.cast::<SqliteCursor>() {
        return cursor
            .get()
            .next_row()
            .map(|row| Bound::new(cursor.py(), row).map(Bound::into_any))
            .transpose();
    }
    let row: Bound<'py, PyAny> = cursor.call_method0("fetchone")?;
    Ok((!row.is_none()).then_some(row))
}
