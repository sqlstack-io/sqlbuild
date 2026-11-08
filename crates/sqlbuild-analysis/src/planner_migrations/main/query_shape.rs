//! Parse the conservative shape used for column migration proofs.

use crate::planner_migrations::_helpers::proofs::ast::{canonical, strip_formatting};
use crate::planner_migrations::_helpers::proofs::clauses::{clause_references, positional};
use crate::planner_migrations::_helpers::proofs::encoding::encode;
use crate::planner_migrations::_helpers::proofs::inputs::select_inputs;
use crate::planner_migrations::_helpers::proofs::parse::proof_tree;
use crate::planner_migrations::_helpers::proofs::projection::named_projections;
use crate::planner_migrations::models::QueryShape;
use serde_json::json;

/// Named projections of exactly one plain SELECT, or None when unknowable.
pub fn query_shape(sql: &str, dialect: &str) -> Result<Option<QueryShape>, String> {
    let Some(mut root) = proof_tree(sql, dialect)? else {
        return Ok(None);
    };
    strip_formatting(&mut root);
    if !root
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.contains_key("select"))
    {
        return Ok(None);
    }
    let select = &mut root["select"];
    let Some(projections) = named_projections(select)? else {
        return Ok(None);
    };
    canonical(select);
    let mut body = select.clone();
    if let Some(object) = body.as_object_mut() {
        for key in ["expressions", "order_by", "qualify"] {
            object.shift_remove(key);
        }
    }
    let (input_relations, local_input_columns) = select_inputs(select)?;
    Ok(Some(QueryShape {
        projections,
        body: encode(&body)?,
        alias_clauses: encode(
            &json!({"order_by": select["order_by"], "qualify": select["qualify"]}),
        )?,
        positional: positional(select),
        input_relations,
        local_input_columns,
        clause_references: clause_references(select),
    }))
}
