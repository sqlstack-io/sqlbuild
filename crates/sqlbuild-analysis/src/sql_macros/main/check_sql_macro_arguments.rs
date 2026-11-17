use rayon::ThreadPool;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::sql_macros::_helpers::typing::probes::{
    ProbePlan, check_findings, model_probes, probe_plan,
};
use crate::sql_macros::models::{SqlMacroCheck, SqlMacroFinding};
use crate::sql_macros::types::{ArgumentCheckInput, ProbeColumns};

/// Each model's call arguments its analysed SQL shows do not fit, checked on `pool`.
#[must_use]
pub fn check_sql_macro_arguments(
    dialect: &str,
    models: &[ArgumentCheckInput<'_>],
    pool: &ThreadPool,
) -> Vec<Vec<SqlMacroFinding>> {
    pool.install(|| {
        models
            .par_iter()
            .map(|(sql, schema, checks)| {
                let plan: ProbePlan = probe_plan(sql, checks);
                let columns: Vec<ProbeColumns> = model_probes(dialect, &plan.probes, schema);
                model_findings(&plan, checks, &columns, dialect)
            })
            .collect()
    })
}

fn model_findings(
    plan: &ProbePlan,
    checks: &[SqlMacroCheck],
    columns: &[ProbeColumns],
    dialect: &str,
) -> Vec<SqlMacroFinding> {
    checks
        .iter()
        .zip(&plan.evidence)
        .flat_map(|(check, evidence)| check_findings((check, evidence), columns, dialect))
        .collect()
}
