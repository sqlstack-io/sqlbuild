//! Model, function and SQL-hook orchestration through adapter-owned rendering.

use crate::bindings::_helpers::attachments::cursor_intrinsics::resolve_cursor_intrinsics;
use crate::bindings::_helpers::attachments::reference_markers::assert_no_unresolved_sql_markers;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::compile_attachment::cursor_roles::resolve_cursor_input_roles;
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::cursor::scalars::sentinel_to_token;
use crate::bindings::_helpers::model_config::typed_config::{
    config_node_of, config_text_is, config_text_object, config_value, model_values,
};
use crate::bindings::_helpers::native_host::synced_copy::SyncedCopyAdapter;
use crate::bindings::_helpers::planner_entries::models::ModelPlanContext;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_resolution::cursor_model::{
    compute_model_cursor_bounds, unbounded,
};
use crate::bindings::_helpers::planner_resolution::models::{CursorFiltering, ModelSqlRequest};
use crate::bindings::_helpers::planner_resolution::refs::{
    resolve_dbt_ref_references, resolve_refs, resolve_table_function_references,
    resolve_udf_references,
};
use crate::bindings::_helpers::planner_resolution::sources::resolve_sources;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{PyErr, pyfunction, wrap_pyfunction};
use sqlbuild_model_config::config_values::models::ConfigNode;

const APPEND_STRATEGY: &str = "append";

/// What resolving one model's, hook's or function's references reads.
pub(crate) struct Resolution<'py> {
    pub(crate) adapter: Bound<'py, PyAny>,
    external: Bound<'py, PyAny>,
    source_map: Bound<'py, PyAny>,
    source_warehouse_columns: Bound<'py, PyAny>,
    model_locations: Bound<'py, PyAny>,
    seed_locations: Bound<'py, PyAny>,
    function_locations: Bound<'py, PyAny>,
}

impl<'py> Resolution<'py> {
    /// Resolution against a `ModelPlanContext`'s relations.
    pub(crate) fn of_context(
        adapter: Bound<'py, PyAny>,
        context: &Bound<'py, PyAny>,
        external: Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            adapter,
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

    fn resolve(&self, sql: &str, filtering: &CursorFiltering<'py>) -> PyResult<String> {
        let py = self.adapter.py();
        let resolved: String = resolve_sources(
            sql,
            (&self.source_map, &self.source_warehouse_columns),
            &self.adapter,
            filtering,
        )?;
        let resolved: String = resolve_refs(
            &resolved,
            (&self.model_locations, &self.seed_locations),
            &self.adapter,
            filtering,
        )?;
        let resolved: String = resolve_dbt_ref_references(&resolved, Some(&self.external))?;
        let locations = if self.function_locations.is_truthy()? {
            self.function_locations.clone()
        } else {
            PyDict::new(py).into_any()
        };
        let resolved: String = resolve_udf_references(&resolved, &locations, &self.adapter)?;
        resolve_table_function_references(&resolved, &locations, &self.adapter)
    }

    fn assert_resolved(&self, sql: &str, label: &str) -> PyResult<()> {
        let py = self.adapter.py();
        assert_no_unresolved_sql_markers(
            py,
            &PyString::new(py, sql),
            &format!("{label} planned SQL"),
        )
    }

    fn unbounded(&self, sql: Bound<'py, PyAny>, label: &str) -> PyResult<String> {
        let py = self.adapter.py();
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

    fn hook(&self, value: Bound<'py, PyAny>, label: &str) -> PyResult<Bound<'py, PyAny>> {
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

#[pyfunction(name = "resolve_model_sql")]
#[pyo3(signature = (**kwargs))]
fn resolve_model_sql_of<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_model_sql",
        kwargs,
        (
            &[
                "adapter",
                "model",
                "snapshot",
                "context",
                "backfill",
                "full_refresh",
                "cursor_overrides",
            ],
            &[
                "external_sql_reference_resolver",
                "replaces_relation",
                "full_rebuild_cause",
            ],
        ),
    )?;
    let context = args.value("context")?;
    let resolution = Resolution::of_context(
        args.value("adapter")?,
        &context,
        args.value("external_sql_reference_resolver")?,
    )?;
    let overrides = args.value("cursor_overrides")?;
    let cause = args.value("full_rebuild_cause")?;
    let request = ModelSqlRequest {
        model: args.value("model")?,
        snapshot: args.value("snapshot")?,
        backfill: args.value("backfill")?,
        full_refresh: args.flag("full_refresh")?,
        cursor_overrides: (
            native_field(&overrides, "start_cursor_override")?,
            native_field(&overrides, "end_cursor_override")?,
        ),
        replaces_relation: args.flag("replaces_relation")?,
        full_rebuild_cause: if cause.is_truthy()? {
            Some(cause.extract()?)
        } else {
            None
        },
    };
    let sql: String = resolve_model_sql(&resolution, &context, &request)?;
    Ok(PyString::new(py, &sql).into_any())
}

/// One model's planned SQL: references resolved, cursor inputs filtered and intrinsics bound.
pub(crate) fn resolve_model_sql<'py>(
    resolution: &Resolution<'py>,
    context: &Bound<'py, PyAny>,
    request: &ModelSqlRequest<'py>,
) -> PyResult<String> {
    let model = &request.model;
    let py = model.py();
    let cursor_type = config_text_object(py, &model_values(model)?, "cursor_type")?
        .unwrap_or_else(|| py.None().into_bound(py));
    let bounds = compute_model_cursor_bounds(request, context)?;
    let roles = resolve_cursor_input_roles(model)?;
    let filtering = CursorFiltering {
        bounds: bounds.clone(),
        filter_inputs: native_field(&roles, "filter_inputs")?,
        cursor_type: cursor_type.clone(),
        lower_bound_inclusive: append_cursor_inclusive(model)?,
    };
    let mut sql = resolution.resolve(
        &model_reads::query_sql(model)?.extract::<String>()?,
        &filtering,
    )?;
    let (_, uses_intrinsics) = resolve_cursor_intrinsics(py, &sql, None, None)?;
    let name: String = model_reads::model_name(model)?;
    if uses_intrinsics {
        let cause = match &request.full_rebuild_cause {
            Some(cause) => Some(cause.clone()),
            None if request.full_refresh => Some("--full-refresh".to_owned()),
            None => None,
        };
        let bounds = match bounds {
            bounds if !bounds.is_none() => bounds,
            _ if cause.is_none()
                && resolution.adapter.is_instance_of::<SyncedCopyAdapter>()
                && native_field(&roles, "watermark_inputs")?.is_truthy()? =>
            {
                unbounded(py)?
            }
            _ => return Err(unresolved_bounds_error(py, &name, cause.as_deref())?),
        };
        sql = intrinsic_bounds(&sql, &bounds, &cursor_type, &resolution.adapter)?;
    }
    resolution.assert_resolved(&sql, &format!("Model '{name}'"))?;
    Ok(sql)
}

/// Effective append cursor lower-bound inclusivity.
fn append_cursor_inclusive(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    let values = model_values(model)?;
    let py: Python<'_> = model.py();
    let append: bool = config_text_is(py, &values, "incremental_strategy", APPEND_STRATEGY)?;
    let inclusive: bool = match *config_node_of(&values, "append_cursor_inclusive")? {
        ConfigNode::Bool(flag) => flag,
        _ => true,
    };
    Ok(!append || inclusive)
}

/// `__cursor_start()` / `__cursor_end()` replaced with the adapter's typed bound literals.
fn intrinsic_bounds<'py>(
    sql: &str,
    bounds: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let py = adapter.py();
    let literal = |field: &str| -> PyResult<String> {
        call_with(
            &native_field(adapter, "render_cursor_bound_literal")?,
            &[
                (
                    "value",
                    PyString::new(py, &sentinel_to_token(&native_field(bounds, field)?)?)
                        .into_any(),
                ),
                ("cursor_type", cursor_type.clone()),
            ],
        )?
        .extract()
    };
    let start_sql = literal("start")?;
    let end_sql = literal("end")?;
    Ok(resolve_cursor_intrinsics(py, sql, Some(&start_sql), Some(&end_sql))?.0)
}

fn unresolved_bounds_error(py: Python<'_>, name: &str, cause: Option<&str>) -> PyResult<PyErr> {
    let error = error_class(py, ErrorId::PlannerInput)?;
    let kwargs = PyDict::new(py);
    let message = if let Some(cause) = cause {
        kwargs.set_item("help", "rebuild it with an explicit cursor interval (--start-cursor-ts and --end-cursor-ts, or the -int forms), or make it a microbatch model so a full rebuild can run batch by batch")?;
        format!(
            "Model '{name}' was given a FULL backfill ({cause}), but it uses cursor intrinsics and a non-microbatch full rebuild has no cursor interval"
        )
    } else {
        format!("Model '{name}' uses cursor intrinsics, but cursor bounds could not be resolved")
    };
    Ok(PyErr::from_value(error.call((message,), Some(&kwargs))?))
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_function_sql<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_function_sql",
        kwargs,
        (
            &[
                "adapter",
                "function",
                "model_locations",
                "seed_locations",
                "function_locations",
                "source_map",
                "source_warehouse_columns",
                "star_exclude_keyword",
            ],
            &[],
        ),
    )?;
    let resolution = Resolution {
        adapter: args.value("adapter")?,
        external: py.None().into_bound(py),
        source_map: args.value("source_map")?,
        source_warehouse_columns: args.value("source_warehouse_columns")?,
        model_locations: args.value("model_locations")?,
        seed_locations: args.value("seed_locations")?,
        function_locations: args.value("function_locations")?,
    };
    let function = args.value("function")?;
    let name: String = function_reads::name(&function)?.extract()?;
    let sql: String = resolution.unbounded(
        function_reads::body_sql(&function)?,
        &format!("Function '{name}'"),
    )?;
    Ok(PyString::new(py, &sql).into_any())
}

#[pyfunction]
#[pyo3(signature = (*, model, adapter, context, external_sql_reference_resolver))]
fn resolve_model_hook_entries<'py>(
    model: &Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
    context: Bound<'py, PyAny>,
    external_sql_reference_resolver: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let resolution = Resolution::of_context(adapter, &context, external_sql_reference_resolver)?;
    model_hook_entries(model, &resolution)
}

/// One model's pre and post hooks with their SQL resolved.
pub(crate) fn model_hook_entries<'py>(
    model: &Bound<'py, PyAny>,
    resolution: &Resolution<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let values = model_values(model)?;
    let name: String = model_reads::model_name(model)?;
    let mut hooks: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in ["pre_hooks", "post_hooks"] {
        hooks.push(resolution.hook(
            config_value(py, &values, key)?,
            &format!("Model '{name}' {key}"),
        )?);
    }
    PyTuple::new(model.py(), hooks)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_model_sql_of, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_function_sql, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_hook_entries, module)?)
}
