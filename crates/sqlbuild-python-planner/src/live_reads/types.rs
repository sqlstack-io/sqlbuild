//! Type aliases of live warehouse reads.

use pyo3::prelude::{Bound, PyAny};

/// Each answered read's position with its row, and the positions of reads whose batch failed.
pub type CursorBatchOutcomes<'py> = (Vec<(usize, Bound<'py, PyAny>)>, Vec<usize>);
