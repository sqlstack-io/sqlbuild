//! Display-only plans for external plan surfaces when warehouse planning cannot resolve.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::graph::views::edge_dict;
use crate::bindings::_helpers::model_config::typed_config::{config_text_object, model_values};
use crate::bindings::_helpers::planner_entries::values::{
    boolean, native, none, planner_model, planner_type, string,
};
use crate::bindings::_helpers::planner_identity::graph::execution_deps;
use crate::bindings::_helpers::planner_phases::full_refresh::resolve_model_full_refresh;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_strategy::strategy::materialization_of;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::graph::main::downstream_edges::downstream_edges;

const VIEW: &str = "view";
const INCREMENTAL: &str = "incremental";
const CUSTOM: &str = "custom";

fn display_action(
    materialization_type: &Bound<'_, PyAny>,
    full_refresh: bool,
) -> PyResult<&'static str> {
    Ok(if materialization_type.eq(VIEW)? {
        "CREATE_VIEW"
    } else if materialization_type.eq(INCREMENTAL)? {
        if full_refresh {
            "CREATE_TABLE"
        } else {
            "INCREMENTAL_APPEND"
        }
    } else if materialization_type.eq(CUSTOM)? {
        "CUSTOM"
    } else {
        "CREATE_TABLE"
    })
}

fn config<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    Ok(config_text_object(py, &model_values(model)?, key)?
        .unwrap_or_else(|| py.None().into_bound(py)))
}

fn display_entry<'py>(
    model: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let materialization_type = materialization_of(model)?;
    let effective = object(
        py,
        resolve_model_full_refresh(model, boolean(py, full_refresh)?.is_truthy()?)?,
    )?
    .is_truthy()?;
    let query_sql = model_reads::query_sql(model)?;
    let custom_name = if materialization_type.eq(CUSTOM)? {
        config(model, "materialized")?
    } else {
        none(py)
    };
    planner_model(
        py,
        RecordId::ModelPlanEntry,
        &[
            ("key", model_reads::model_key(model)?),
            ("name", model_reads::name_object(model)?),
            ("relative_path", native_field(model, "relative_path")?),
            (
                "action",
                planner_type(
                    py,
                    RecordId::PlanAction,
                    display_action(&materialization_type, effective)?,
                )?,
            ),
            ("materialization_type", materialization_type),
            (
                "reason",
                planner_type(
                    py,
                    RecordId::PlanReason,
                    if effective {
                        "FULL_REFRESH"
                    } else {
                        "NO_CHANGE"
                    },
                )?,
            ),
            ("destination", model_reads::destination(model)?),
            ("fingerprint_query_sql", query_sql.clone()),
            ("resolved_sql", query_sql),
            ("logical_ddl", string(py, "")),
            (
                "incremental_strategy",
                config(model, "incremental_strategy")?,
            ),
            ("incremental_mode", config(model, "incremental_mode")?),
            ("cursor_column", config(model, "cursor_column")?),
            ("cursor_type", config(model, "cursor_type")?),
            (
                "backfill",
                native(
                    py,
                    RecordId::BackfillResult,
                    &[(
                        "action",
                        planner_type(
                            py,
                            RecordId::BackfillAction,
                            if effective { "FULL" } else { "FORWARD_ONLY" },
                        )?,
                    )],
                )?,
            ),
            ("custom_materialization_name", custom_name),
        ],
    )
}

/// Build a display-only plan when warehouse planning cannot fully resolve.
#[pyfunction]
#[pyo3(signature = (*, project, selected_model_names, full_refresh))]
pub(crate) fn build_display_only_sqlbuild_plan<'py>(
    project: &Bound<'py, PyAny>,
    selected_model_names: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let selected = PyFrozenSet::new(
        py,
        selected_model_names
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if selected.contains(model_reads::name_object(&model)?)? {
            entries.push(display_entry(&model, full_refresh)?);
        }
    }
    let keys: Vec<Bound<'py, PyAny>> = entries
        .iter()
        .map(|entry| native_field(entry, "key"))
        .collect::<PyResult<_>>()?;
    let upstream = execution_deps(project)?;
    let cache = KeyCache::new(py)?;
    let downstream = edge_dict(&cache, &downstream_edges(&upstream))?.into_any();
    let upstream = edge_dict(&cache, &upstream)?.into_any();
    planner_model(
        py,
        RecordId::PlanOutput,
        &[
            ("execution_order", PyTuple::new(py, &keys)?.into_any()),
            ("model_entries", PyTuple::new(py, entries)?.into_any()),
            ("selected_keys", PyFrozenSet::new(py, keys)?.into_any()),
            ("upstream_deps", upstream),
            ("downstream_deps", downstream),
        ],
    )
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_display_only_sqlbuild_plan, module)?)
}
