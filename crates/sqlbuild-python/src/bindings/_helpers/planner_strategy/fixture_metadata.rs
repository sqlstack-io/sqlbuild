//! A model's fixture relation metadata from its inferred and declared columns.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_fixtures::main::merge_metadata::merge_metadata;
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::model_reads::{inferred_rows, star_flags};
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_strategy::constants::ENFORCED_CONTRACT;
use crate::bindings::_helpers::planner_strategy::fixture_values::{columns, folded_name};
use crate::bindings::_helpers::planner_strategy::models::FixtureRelation;
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_project::types::InferredRow;

const CONTRACT_KEY: &str = "contract";

/// The model's merged inferred and declared columns, authoritative unless its analysis saw a star.
pub(crate) fn model_fixture_relation(model: &Bound<'_, PyAny>) -> PyResult<FixtureRelation> {
    let inferred: Option<Vec<InferredRow>> = inferred_rows(model)?;
    let schema = model_reads::schema_entry(model)?;
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
