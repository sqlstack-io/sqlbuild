//! The sorted texts of a set of strings.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The sorted texts of a set of strings.
pub fn sorted_texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut texts: Vec<String> = values
        .try_iter()?
        .map(|value| value?.extract::<String>())
        .collect::<PyResult<_>>()?;
    texts.sort_unstable();
    Ok(texts)
}
