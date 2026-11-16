//! The project layer of the `sqlbuild._native` bindings: the compiled project and its views, graph, references, lineage, fingerprints, semantic shapes, planner scope, selection and strategy, and SQL test planning.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod analysis_session;
pub mod auditing;
pub mod compile_attachment;
pub mod compiled_project;
pub mod compiled_views;
pub mod contracts;
pub mod fingerprints;
pub mod graph;
pub mod lineage;
pub mod native_host;
pub mod planner_identity;
pub mod planner_phases;
pub mod planner_scope;
pub mod planner_selection;
pub mod planner_strategy;
pub mod project_snapshot;
pub mod references;
pub mod relation_naming;
pub mod resource_views;
pub mod semantic_shapes;
pub mod sql_test_glue;
pub mod type_system;
