//! Public records of planner resolution.

use crate::planner_entries::models::ModelPlanContext;
use crate::planner_phases::main::inputs::class_field::class_field;
use crate::planner_resolution::_helpers::references::refs::resolve_dbt_ref_references;
use crate::planner_resolution::_helpers::references::refs::resolve_refs;
use crate::planner_resolution::_helpers::references::refs::resolve_table_function_references;
use crate::planner_resolution::_helpers::references::refs::resolve_udf_references;
use crate::planner_resolution::_helpers::references::sources::resolve_sources;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::{Bound, PyAny};
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::attachments::main::assert_no_unresolved_sql_markers::assert_no_unresolved_sql_markers;

/// The planner inputs that decide one model's plan-time cursor window.
#[derive(Debug)]
pub struct CursorWindowInputs<'py> {
    pub snapshot: Bound<'py, PyAny>,
    pub backfill: Bound<'py, PyAny>,
    pub overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub runtime_owned: bool,
    pub policies: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub invocation_time: Bound<'py, PyAny>,
    pub full_refresh: bool,
    pub replaces_relation: bool,
    pub cursor_input_relations: Bound<'py, PyAny>,
}

/// The cursor window that filters the cursor inputs one resolution reads.
#[derive(Debug)]
pub(crate) struct CursorFiltering<'py> {
    pub bounds: Bound<'py, PyAny>,
    pub filter_inputs: Bound<'py, PyAny>,
    pub cursor_type: Bound<'py, PyAny>,
    pub lower_bound_inclusive: bool,
}

/// One model's SQL resolution decisions.
#[derive(Debug)]
pub struct ModelSqlRequest<'py> {
    pub model: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub backfill: Bound<'py, PyAny>,
    pub full_refresh: bool,
    pub cursor_overrides: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    pub replaces_relation: bool,
    pub full_rebuild_cause: Option<String>,
}

/// What resolving one model's, hook's or function's references reads.
#[derive(Debug)]
pub struct Resolution<'py> {
    pub(crate) dialect: Bound<'py, Dialect>,
    pub(crate) external: Bound<'py, PyAny>,
    pub(crate) source_map: Bound<'py, PyAny>,
    pub(crate) source_warehouse_columns: Bound<'py, PyAny>,
    pub(crate) model_locations: Bound<'py, PyAny>,
    pub(crate) seed_locations: Bound<'py, PyAny>,
    pub(crate) function_locations: Bound<'py, PyAny>,
}

impl<'py> Resolution<'py> {
    /// Resolution against a `ModelPlanContext`'s relations.
    pub fn of_context(
        dialect: Bound<'py, Dialect>,
        context: &Bound<'py, PyAny>,
        external: Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            dialect,
            external,
            source_map: class_field(context, |c: &ModelPlanContext| &c.source_map)?,
            source_warehouse_columns: class_field(context, |c: &ModelPlanContext| {
                &c.source_warehouse_columns
            })?,
            model_locations: class_field(context, |c: &ModelPlanContext| &c.model_locations)?,
            seed_locations: class_field(context, |c: &ModelPlanContext| &c.seed_locations)?,
            function_locations: class_field(context, |c: &ModelPlanContext| &c.function_locations)?,
        })
    }

    pub(crate) fn resolve(&self, sql: &str, filtering: &CursorFiltering<'py>) -> PyResult<String> {
        let py = self.dialect.py();
        let resolved: String = resolve_sources(
            sql,
            (&self.source_map, &self.source_warehouse_columns),
            &self.dialect,
            filtering,
        )?;
        let resolved: String = resolve_refs(
            &resolved,
            (&self.model_locations, &self.seed_locations),
            &self.dialect,
            filtering,
        )?;
        let resolved: String = resolve_dbt_ref_references(&resolved, Some(&self.external))?;
        let locations = if self.function_locations.is_truthy()? {
            self.function_locations.clone()
        } else {
            PyDict::new(py).into_any()
        };
        let resolved: String = resolve_udf_references(&resolved, &locations, &self.dialect)?;
        resolve_table_function_references(&resolved, &locations, &self.dialect)
    }

    pub(crate) fn assert_resolved(&self, sql: &str, label: &str) -> PyResult<()> {
        let py = self.dialect.py();
        assert_no_unresolved_sql_markers(
            py,
            &PyString::new(py, sql),
            &format!("{label} planned SQL"),
        )
    }

    pub(crate) fn unbounded(&self, sql: Bound<'py, PyAny>, label: &str) -> PyResult<String> {
        let py = self.dialect.py();
        let resolved = self.resolve(
            &sql.extract::<String>()?,
            &CursorFiltering {
                bounds: py.None().into_bound(py),
                filter_inputs: PyDict::new(py).into_any(),
                cursor_type: py.None().into_bound(py),
                lower_bound_inclusive: true,
            },
        )?;
        self.assert_resolved(&resolved, label)?;
        Ok(resolved)
    }

    pub(crate) fn hook(
        &self,
        value: Bound<'py, PyAny>,
        label: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        if value.is_instance(&record_class(py, RecordId::SqlHookEntry)?)? {
            let kwargs = PyDict::new(py);
            kwargs.set_item(
                "statement",
                self.unbounded(native_field(&value, "statement")?, label)?,
            )?;
            return replaced_keywords(&value, &kwargs);
        }
        let is_list = value.is_instance_of::<PyList>();
        if is_list || value.is_instance_of::<PyTuple>() {
            let mut resolved: Vec<Bound<'py, PyAny>> = Vec::new();
            for (index, item) in value.try_iter()?.enumerate() {
                resolved.push(self.hook(item?, &format!("{label}[{index}]"))?);
            }
            return if is_list {
                Ok(PyList::new(py, resolved)?.into_any())
            } else {
                Ok(PyTuple::new(py, resolved)?.into_any())
            };
        }
        Ok(value)
    }
}
