//! The front layer of the `sqlbuild._native` bindings: discovery, model configuration, scopes, rendering and SQL text.

#![forbid(unsafe_code)]

pub mod attachments;
pub mod authored_values;
pub mod command_runtime;
pub mod compile_attachment;
pub mod compile_driver;
pub mod contracts;
pub mod cost_context;
pub mod cursor;
pub mod discovered_sets;
pub mod discovery;
pub mod execution_history;
pub mod macro_bridge;
pub mod model_config;
pub mod native_host;
pub mod refactor;
pub mod render;
pub mod resource_costs;
pub mod scopes;
pub mod sql_test_glue;
pub mod sqltext;
pub mod state_rows;
