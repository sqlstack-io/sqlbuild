//! The compile layer of the `sqlbuild._native` bindings: the compile driver and command, input loops, project assembly, analysis and the analysis session, rules, lineage, semantic checks, compile deltas and outputs, the DAG artifact, manifest, frontier and auditing.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod analysis_session;
pub mod auditing;
pub mod compile_attachment;
pub mod compile_command;
pub mod compile_delta;
pub mod compile_driver;
pub mod compile_outputs;
pub mod dag_artifact;
pub mod frontier;
pub mod hand_built;
pub mod input_loops;
pub mod lineage;
pub mod manifest;
pub mod project_assembly;
pub mod rules;
pub mod scopes;
pub mod semantic_checks;
pub mod sql_test_glue;
