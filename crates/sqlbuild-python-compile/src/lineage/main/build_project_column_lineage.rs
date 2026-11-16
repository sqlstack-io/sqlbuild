//! The project's column lineage graph in `mode`, or `None` without SQL analysis.

use crate::lineage::_helpers::graph::project_lineage::FAST_MODE;
use crate::lineage::_helpers::graph::project_lineage::RICH_MODE;
use crate::lineage::_helpers::graph::project_lineage::fast_lineage;
use crate::lineage::_helpers::graph::project_lineage::rich_lineage;
use crate::lineage::_helpers::graph::project_lineage::selected_models;
use crate::lineage::models::LineageClasses;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's column lineage graph in `mode`, or `None` without SQL analysis.
#[pyfunction]
pub fn build_project_column_lineage<'py>(
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    mode: &Bound<'py, PyAny>,
    model_names: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Py<PyAny>>> {
    let mode: String = mode.str()?.extract()?;
    if mode != FAST_MODE && mode != RICH_MODE {
        return Ok(None);
    }
    let classes: LineageClasses<'py> = LineageClasses::new(project.py())?;
    if !native_field(project, "settings")?
        .getattr("sql_analysis")?
        .is_truthy()?
    {
        return Ok(None);
    }
    let models: Vec<Bound<'py, PyAny>> = selected_models(project, model_names)?;
    let lineage: Bound<'py, PyAny> = if mode == FAST_MODE {
        fast_lineage(&classes, project, dialect, &models)?
    } else {
        rich_lineage(&classes, project, dialect, &models)?
    };
    Ok(Some(lineage.unbind()))
}
