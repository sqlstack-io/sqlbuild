//! The facts of `inputs`, with the Python nodes the facade has described.

use crate::discovery::_helpers::project::project_facts::audit_fact;
use crate::discovery::_helpers::project::project_facts::csv_field_limit;
use crate::discovery::_helpers::project::project_facts::declarations_fact;
use crate::discovery::_helpers::project::project_facts::each;
use crate::discovery::_helpers::project::project_facts::model_fact;
use crate::discovery::_helpers::project::project_facts::named;
use crate::discovery::_helpers::project::project_facts::node_fact;
use crate::discovery::_helpers::project::project_facts::schema_fact;
use crate::discovery::_helpers::project::project_facts::seed_file_fact;
use crate::discovery::_helpers::project::project_facts::source_file_fact;
use crate::discovery::_helpers::project::project_facts::stem_named;
use crate::discovery::_helpers::project::project_facts::test_file_fact;
use crate::discovery::models::NodeRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_discovery::project_validation::models::{
    NamedPath, ProjectFacts, ScopedDeclarationsFact,
};
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The facts of `inputs`, with the Python nodes the facade has described.
pub fn project_facts(
    py: Python<'_>,
    inputs: &Bound<'_, PyAny>,
    nodes: Vec<NodeRow>,
) -> PyResult<ProjectFacts> {
    let mut declaration_files: Vec<ScopedDeclarationsFact> =
        each(inputs, "enum_files", declarations_fact)?;
    declaration_files.extend(each(inputs, "constant_files", declarations_fact)?);
    let mut model_schemas: Vec<NamedPath> = Vec::new();
    for schema in native_field(inputs, "model_schema_files")?.try_iter()? {
        model_schemas.extend(each(&schema?, "declarations", named)?);
    }
    Ok(ProjectFacts {
        models: each(inputs, "model_files", model_fact)?,
        sql_functions: each(inputs, "sql_function_files", stem_named)?,
        python_functions: each(inputs, "python_function_files", stem_named)?,
        sql_hooks: each(inputs, "sql_hook_files", named)?,
        seeds: each(inputs, "seed_files", seed_file_fact)?,
        schemas: each(inputs, "schema_files", |schema| schema_fact(py, schema))?,
        sources: each(inputs, "source_files", source_file_fact)?,
        tests: each(inputs, "test_files", test_file_fact)?,
        scenarios: each(inputs, "scenario_files", named)?,
        audits: each(inputs, "audit_files", audit_fact)?,
        declaration_files,
        model_schemas,
        materializations: each(inputs, "materialization_files", named)?,
        nodes: nodes.into_iter().map(node_fact).collect::<PyResult<_>>()?,
        hook_functions: each(inputs, "hook_functions", named)?,
        event_exporters: each(inputs, "event_exporters", named)?,
        providers: each(inputs, "providers", named)?,
        path_default_keys: native_field(inputs, "project_config")?
            .getattr("path_defaults")?
            .try_iter()?
            .map(|key| key?.extract())
            .collect::<PyResult<_>>()?,
        project_model_paths: injection().map(|injection| injection.project_model_paths.clone()),
        csv_field_limit: Some(csv_field_limit(py)?),
        orchestration: Vec::new(),
    })
}
