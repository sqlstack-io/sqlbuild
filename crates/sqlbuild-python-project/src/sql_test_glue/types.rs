//! Public traits and type aliases of sql test glue.

use pyo3::prelude::{Bound, PyAny};
use std::collections::BTreeMap;

pub(crate) type ValidatedTests<'py> = (Vec<Bound<'py, PyAny>>, BTreeMap<usize, Bound<'py, PyAny>>);
