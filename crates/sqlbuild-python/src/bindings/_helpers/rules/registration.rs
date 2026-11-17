//! Register the native rules, lint and DSL header functions.

use pyo3::prelude::{Bound, PyModule, PyResult};
use pyo3::types::PyModuleMethods;

use sqlbuild_python_compile::rules::main::checking::register_dsl_headers::register_dsl_headers;
use sqlbuild_python_compile::rules::main::checking::register_evaluation::register_evaluation;
use sqlbuild_python_compile::rules::main::checking::register_lint::register_lint;
use sqlbuild_python_compile::rules::main::checking::register_project_tree_queries::register_project_tree_queries;
use sqlbuild_python_compile::rules::main::checking::register_results::register_results;
use sqlbuild_python_compile::rules::main::configuration::register_catalogue::register_catalogue;
use sqlbuild_python_compile::rules::main::configuration::register_guidance::register_guidance;
use sqlbuild_python_compile::rules::main::configuration::register_rules_config::register_rules_config;
use sqlbuild_python_compile::rules::main::configuration::register_selection::register_selection;
use sqlbuild_python_compile::rules::main::custom::register_custom_evaluation::register_custom_evaluation;
use sqlbuild_python_compile::rules::main::custom::register_custom_host_observation::register_custom_host_observation;
use sqlbuild_python_compile::rules::main::custom::register_custom_rule_cache::register_custom_rule_cache;
use sqlbuild_python_compile::rules::main::custom::register_custom_rule_evidence::register_custom_rule_evidence;
use sqlbuild_python_compile::rules::main::fact::register_fact_encoder::register_fact_encoder;
use sqlbuild_python_compile::rules::main::fix::compile_facts_function::compile_facts_function;
use sqlbuild_python_compile::rules::main::fix::fix_verdict_function::fix_verdict_function;
use sqlbuild_python_compile::rules::main::fix::plan_rule_fixes_function::plan_rule_fixes_function;
use sqlbuild_python_compile::rules::main::lint::lint_file_role_function::lint_file_role_function;
use sqlbuild_python_compile::rules::main::lint::prepare_lint_body_function::prepare_lint_body_function;
use sqlbuild_python_compile::rules::main::lint::register_early_sql_lint::register_early_sql_lint;
use sqlbuild_python_compile::rules::main::lint::register_lint_expansion::register_lint_expansion;
use sqlbuild_python_compile::rules::main::lint::register_lint_sql_bodies::register_lint_sql_bodies;
use sqlbuild_python_compile::rules::main::lint::register_lint_text::register_lint_text;
use sqlbuild_python_compile::rules::main::lint::register_run_lint::register_run_lint;
use sqlbuild_python_compile::rules::main::rule::register_rule_api::register_rule_api;
use sqlbuild_python_compile::rules::main::rule::register_rule_cache::register_rule_cache;
use sqlbuild_python_compile::rules::main::rule::register_rule_dispatch::register_rule_dispatch;
use sqlbuild_python_compile::rules::main::rule::register_rule_plan::register_rule_plan;
use sqlbuild_python_compile::rules::main::rule::register_rule_run::register_rule_run;
use sqlbuild_python_compile::rules::main::sql::register_fixture_formatting::register_fixture_formatting;
use sqlbuild_python_compile::rules::main::sql::register_sql_document::register_sql_document;
use sqlbuild_python_compile::rules::main::sql::register_sql_rules::register_sql_rules;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_rule_api(module)?;
    register_sql_document(module)?;
    register_project_tree_queries(module)?;
    module.add_function(lint_file_role_function(module)?)?;
    module.add_function(prepare_lint_body_function(module)?)?;
    module.add_function(plan_rule_fixes_function(module)?)?;
    module.add_function(compile_facts_function(module)?)?;
    module.add_function(fix_verdict_function(module)?)?;
    register_evaluation(module)?;
    register_lint(module)?;
    register_lint_text(module)?;
    register_fixture_formatting(module)?;
    register_lint_expansion(module)?;
    register_lint_sql_bodies(module)?;
    register_dsl_headers(module)?;
    register_custom_rule_evidence(module)?;
    register_rule_cache(module)?;
    register_rule_run(module)?;
    register_rule_plan(module)?;
    register_sql_rules(module)?;
    register_early_sql_lint(module)?;
    register_custom_rule_cache(module)?;
    register_catalogue(module)?;
    register_rules_config(module)?;
    register_run_lint(module)?;
    register_selection(module)?;
    register_results(module)?;
    register_guidance(module)?;
    register_custom_evaluation(module)?;
    register_fact_encoder(module)?;
    register_custom_host_observation(module)?;
    register_rule_dispatch(module)
}
