//! A model's fixture relation metadata from its inferred and declared columns.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_fixtures::main::merge_metadata::merge_metadata;
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;

use crate::compiled_views::main::compiled_models::inferred_rows::inferred_rows;
use crate::compiled_views::main::compiled_models::schema_entry::schema_entry;
use crate::compiled_views::main::compiled_models::star_flags::star_flags;
use crate::planner_strategy::_helpers::fixtures::fixture_values::columns;
use crate::planner_strategy::_helpers::fixtures::fixture_values::folded_name;
use crate::planner_strategy::constants::ENFORCED_CONTRACT;
use crate::planner_strategy::models::FixtureRelation;
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

const CONTRACT_KEY: &str = "contract";

/// The model's merged inferred and declared columns, authoritative unless its analysis saw a star.
pub(crate) fn model_fixture_relation(model: &Bound<'_, PyAny>) -> PyResult<FixtureRelation> {
    let inferred: Option<Vec<InferredRow>> = inferred_rows(model)?;
    let schema = schema_entry(model)?;
    let declared: Vec<FixtureColumn> = if schema.is_none() {
        Vec::new()
    } else {
        columns(&native_field(&schema, "columns")?, false)?
    };
    let mut inferred_columns: Vec<FixtureColumn> = Vec::new();
    for (name, sql_type, _) in inferred.iter().flatten() {
        inferred_columns.push(FixtureColumn {
            name: name.clone(),
            folded_name: folded_name(name),
            sql_type: sql_type.clone(),
            nullable: None,
        });
    }
    let enforced: bool =
        config_value(model.py(), &model_values(model)?, CONTRACT_KEY)?.eq(ENFORCED_CONTRACT)?;
    let authoritative = enforced || (!star_flags(model)?.0 && inferred.is_some());
    Ok(FixtureRelation {
        columns: merge_metadata(inferred_columns, declared),
        authoritative,
    })
}
