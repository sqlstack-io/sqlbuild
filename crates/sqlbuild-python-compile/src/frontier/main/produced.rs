//! The frontier object `produced` for `stage`, captured on request.

use crate::frontier::_helpers::compile_frontier::frontier_stage;
use crate::frontier::_helpers::stage_capture::write_stage_capture;
use crate::frontier::models::FrontierStage;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The frontier object `produced` for `stage`, captured on request.
pub fn produced<'py>(
    stage: FrontierStage,
    produced: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    write_stage_capture(&frontier_stage(produced.py(), stage)?, &produced)?;
    Ok(produced)
}
