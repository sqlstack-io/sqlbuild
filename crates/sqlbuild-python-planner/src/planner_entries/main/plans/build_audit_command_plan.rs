//! Project selected audits from canonical static state.

use crate::planner_entries::_helpers::plans::command_plans::StaticPlanParts;
use crate::planner_entries::_helpers::plans::command_plans::base_plan_output;
use crate::planner_strategy::main::audit_entries::build_selected_audit_entries::build_selected_audit_entries;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::planner_phases::main::object::object;

/// Project selected audits from canonical static state.
#[pyfunction]
#[pyo3(signature = (*, project, dialect, scope, relations))]
pub fn build_audit_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    dialect: &Bound<'py, Dialect>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let materializations = PyDict::new(py);
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        materializations.set_item(
            name_object(&model)?,
            native_field(&materialization_of(&model)?, "value")?,
        )?;
    }
    let audits = object(
        py,
        build_selected_audit_entries(Some(&keywords(
            py,
            &[
                ("project", project.clone()),
                ("dialect", dialect.clone().into_any()),
                ("scope", scope.clone()),
                ("relations", relations.clone()),
                ("model_materializations", materializations.into_any()),
            ],
        )?))?,
    )?;
    base_plan_output(
        project,
        scope,
        relations,
        StaticPlanParts {
            execution_order: None,
            fields: vec![(
                "audit_entries",
                PyTuple::new(py, audits.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any(),
            )],
        },
    )
}
