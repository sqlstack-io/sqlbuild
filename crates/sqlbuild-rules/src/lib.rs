//! Built-in Rules and their configuration, hosting SQLClean's SQL lint, quality Rules and formatter.

#![forbid(unsafe_code)]

pub mod configuration;
pub mod constants;
pub mod custom_host_pool;
pub mod custom_rule_cache;
pub mod dsl_headers;
pub mod engine;
pub mod errors;
pub mod fix_verification;
pub mod lint_text;
pub mod models;
pub mod project_tree;
pub mod query_facts;
pub mod relation_keys;
pub mod rule_cache;
pub mod rule_catalogue;
pub mod rule_results;
pub mod rules;
pub mod scope_metadata;
pub mod types;
