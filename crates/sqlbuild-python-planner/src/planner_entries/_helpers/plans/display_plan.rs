//! Display-only plans for external plan surfaces when warehouse planning cannot resolve.

use crate::planner_entries::_helpers::entries::values::{
    boolean, native, none, planner_model, planner_type, string,
};
use crate::planner_phases::main::rebuilds::resolve_model_full_refresh::resolve_model_full_refresh;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_key::model_key;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::planner_phases::main::object::object;

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

pub(crate) fn display_entry<'py>(
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
    let query_sql =
        sqlbuild_python_project::compiled_views::main::compiled_models::query_sql::query_sql(
            model,
        )?;
    let custom_name = if materialization_type.eq(CUSTOM)? {
        config(model, "materialized")?
    } else {
        none(py)
    };
    planner_model(
        py,
        RecordId::ModelPlanEntry,
        &[
            ("key", model_key(model)?),
            ("name", name_object(model)?),
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
            ("destination", destination(model)?),
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
