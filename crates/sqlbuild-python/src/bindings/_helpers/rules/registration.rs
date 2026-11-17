//! Register the native rules, lint and DSL header functions.

use crate::bindings::_helpers::rules::fix_facts::compile_facts;
use crate::bindings::_helpers::rules::fix_verdict::fix_verdict;
use pyo3::prelude::{Bound, PyModule, PyResult};
use pyo3::{types::PyModuleMethods, wrap_pyfunction};

use crate::bindings::_helpers::rules::catalogue;
use crate::bindings::_helpers::rules::custom_rule_cache;
use crate::bindings::_helpers::rules::custom_rule_evidence;
use crate::bindings::_helpers::rules::dsl_headers;
use crate::bindings::_helpers::rules::early_sql_lint;
use crate::bindings::_helpers::rules::evaluation;
use crate::bindings::_helpers::rules::guidance;
use crate::bindings::_helpers::rules::lint;
use crate::bindings::_helpers::rules::lint_expansion;
use crate::bindings::_helpers::rules::lint_sql_bodies;
use crate::bindings::_helpers::rules::lint_text;
use crate::bindings::_helpers::rules::results;
use crate::bindings::_helpers::rules::rule_api;
use crate::bindings::_helpers::rules::rule_cache;
use crate::bindings::_helpers::rules::rule_plan;
use crate::bindings::_helpers::rules::rule_run;
use crate::bindings::_helpers::rules::rules_config;
use crate::bindings::_helpers::rules::run_lint;
use crate::bindings::_helpers::rules::selection;
use crate::bindings::_helpers::rules::sql_document;
use crate::bindings::_helpers::rules::sql_rules;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    rule_api::register(module)?;
    sql_document::register(module)?;
    crate::bindings::_helpers::rules::project_tree_queries::register(module)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::rules::lint_headers::lint_file_role,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::rules::lint_preparation::prepare_lint_body,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::rules::fix_session::plan_rule_fixes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(compile_facts, module)?)?;
    module.add_function(wrap_pyfunction!(fix_verdict, module)?)?;
    evaluation::register(module)?;
    lint::register(module)?;
    lint_text::register(module)?;
    crate::bindings::_helpers::rules::fixture_formatting::register(module)?;
    lint_expansion::register(module)?;
    lint_sql_bodies::register(module)?;
    dsl_headers::register(module)?;
    custom_rule_evidence::register(module)?;
    rule_cache::register(module)?;
    rule_run::register(module)?;
    rule_plan::register(module)?;
    sql_rules::register(module)?;
    early_sql_lint::register(module)?;
    custom_rule_cache::register(module)?;
    catalogue::register(module)?;
    rules_config::register(module)?;
    run_lint::register(module)?;
    selection::register(module)?;
    results::register(module)?;
    guidance::register(module)?;
    crate::bindings::_helpers::rules::custom_evaluation::register(module)?;
    crate::bindings::_helpers::rules::fact_encoder::register(module)?;
    crate::bindings::_helpers::rules::custom_host_observation::register(module)?;
    crate::bindings::_helpers::rules::rule_dispatch::register(module)
}
