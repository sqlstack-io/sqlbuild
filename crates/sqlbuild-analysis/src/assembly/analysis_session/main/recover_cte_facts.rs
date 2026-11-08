//! Python's CTE pass-through facts and filtered non-null outputs for one query.

use crate::assembly::analysis_session::_helpers::cte_facts::{
    Recovery, RecoveryInput, RecoveryProfile, recovery,
};
use crate::assembly::analysis_session::constants::CTE_FACT_WORKER_STACK_BYTES;
use crate::assembly::analysis_session::models::{CteFactRequest, CteFacts};
use crate::worker_stack::main::run_on_worker_stack::run_on_worker_stack;

/// The facts enrichment recovers for the query, or why they are left to Python.
pub fn recover_cte_facts(request: &CteFactRequest) -> Result<CteFacts, String> {
    run_on_worker_stack(
        CTE_FACT_WORKER_STACK_BYTES,
        "native CTE fact recovery panicked",
        || recovered_facts(request),
    )?
}

fn recovered_facts(request: &CteFactRequest) -> Result<CteFacts, String> {
    let recovered: Recovery = recovery(&RecoveryInput {
        cleaned_sql: &request.cleaned_sql,
        input_schemas: &request.input_schemas,
        recover: request.recover,
        null_filter: request.null_filter,
        profile: RecoveryProfile {
            dialect: &request.dialect,
            function_return_types: &request.function_return_types,
            rules: request.nullability_rules.as_ref(),
            callback: None,
        },
    })?;
    let mut direct_outputs: Vec<String> = recovered.direct_outputs.into_iter().collect();
    direct_outputs.sort_unstable();
    let mut non_null_outputs: Vec<String> = recovered.non_null_outputs.into_iter().collect();
    non_null_outputs.sort_unstable();
    Ok(CteFacts {
        types: recovered.types,
        nullability: recovered
            .nullability
            .into_iter()
            .map(|(name, nullability)| (name, nullability.to_owned()))
            .collect(),
        direct_outputs,
        non_null_outputs,
    })
}
