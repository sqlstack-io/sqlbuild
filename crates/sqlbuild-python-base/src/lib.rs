//! The shared base of the `sqlbuild._native` bindings: native records, Python calls and host entries.

#![forbid(unsafe_code)]

pub mod boundary;
pub mod build_command;
pub mod build_progress;
pub mod cache;
pub mod cli;
pub mod command_runtime;
pub mod compile_context;
pub mod compile_driver;
pub mod compile_outputs;
pub mod contracts;
pub mod diagnostics;
pub mod event_exporting;
pub mod frontier;
pub mod invocation_phases;
pub mod native_host;
pub mod native_threads;
pub mod orchestration_reads;
pub mod planner_identity;
pub mod presentation_text;
pub mod python_values;
pub mod records;
pub mod result_output;
pub mod type_system;
pub mod types;
pub mod warehouse_access;
