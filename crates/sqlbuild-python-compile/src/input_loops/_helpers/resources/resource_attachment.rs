//! The model loop's attachment of the project's sources, functions, SQL tests and audits.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::input_loops::_helpers::resources::audit_loop::model_owner;
use crate::input_loops::_helpers::resources::audit_loop::resource_owner;
use crate::input_loops::_helpers::resources::function_loop::whole_partition_functions;
use crate::input_loops::models::AttachmentOwner;
use crate::input_loops::models::NativeModelLoop;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_project::models::AuditFileRow;
use sqlbuild_python_project::compiled_project::models::AuditSettings;
use sqlbuild_python_project::compiled_project::models::FunctionRow;
use sqlbuild_python_project::compiled_project::models::FunctionSettings;
use sqlbuild_python_project::compiled_project::models::NativeCompiledProject;
use sqlbuild_python_project::compiled_project::models::ScenarioRow;
use sqlbuild_python_project::compiled_project::models::SqlTestSettings;
use sqlbuild_python_project::compiled_project::models::TestFileRow;
use sqlbuild_python_project::compiled_project::types::AttachedSqlTests;
use sqlbuild_python_project::compiled_project::types::SourceRow;
use sqlbuild_python_project::compiled_project::types::SourceSettings;
use sqlbuild_python_project::resource_views::models::NativeAuditInput;
use sqlbuild_python_project::resource_views::models::NativeFunctionInput;
use sqlbuild_python_project::resource_views::models::NativeSourceInput;
use sqlbuild_python_project::resource_views::models::SeedParts;
use sqlbuild_python_project::resource_views::types::AttachedFunction;
use sqlbuild_python_project::resource_views::types::AttachedSource;
use sqlbuild_python_project::resource_views::types::SeedViews;

const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";
const SOURCE_OWNER: &str = "source";
const SEED_OWNER: &str = "seed";

/// Attach every discovered source in order with the kept loop; return each source's view.
pub(crate) fn attach_sources(
    project: &Bound<'_, NativeCompiledProject>,
    rows: Vec<SourceRow>,
    settings: SourceSettings,
) -> PyResult<Vec<NativeSourceInput>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = project.py();
    let (model_loop, host) = project.borrow().attachment_loop(py)?;
    let model_loop: &Bound<'_, NativeModelLoop> = model_loop.bind(py).cast()?;
    model_loop
        .get()
        .run_sources(project, host.bind(py), rows, settings)
}

/// Attach every SQL, then Python, function with the kept loop; return each one's view.
pub(crate) fn attach_functions(
    project: &Bound<'_, NativeCompiledProject>,
    rows: (Vec<FunctionRow>, Vec<FunctionRow>),
    settings: FunctionSettings,
) -> PyResult<Vec<NativeFunctionInput>> {
    let py: Python<'_> = project.py();
    let (model_loop, host) = project.borrow().attachment_loop(py)?;
    let model_loop: &Bound<'_, NativeModelLoop> = model_loop.bind(py).cast()?;
    let models: Vec<(String, Vec<ReferenceRow>)> = project
        .borrow()
        .inputs()
        .iter()
        .map(|record| {
            Ok((
                record.host.model_file.get().relative_text(py)?,
                record.references.clone(),
            ))
        })
        .collect::<PyResult<_>>()?;
    let attached: Vec<AttachedFunction> =
        model_loop
            .get()
            .run_functions(host.bind(py), rows, settings, &models)?;
    let count: usize = attached.len();
    let first: usize = project.borrow_mut().extend_functions(attached);
    Ok((first..first + count)
        .map(|index| NativeFunctionInput::new(project.clone().unbind(), index))
        .collect())
}

/// Attach every SQL test case, then every scenario, with the kept loop.
pub(crate) fn attach_sql_tests(
    project: &Bound<'_, NativeCompiledProject>,
    files: Vec<TestFileRow>,
    scenarios: Vec<ScenarioRow>,
    settings: SqlTestSettings,
) -> PyResult<AttachedSqlTests> {
    let py: Python<'_> = project.py();
    let (model_loop, host) = project.borrow().attachment_loop(py)?;
    let model_loop: &Bound<'_, NativeModelLoop> = model_loop.bind(py).cast()?;
    let arities: HashMap<String, usize> = project
        .borrow()
        .functions()
        .iter()
        .filter(|record| !record.attached.return_columns.is_empty())
        .map(|record| {
            (
                record.attached.name.clone(),
                record.attached.arguments.len(),
            )
        })
        .collect();
    let whole_partition: HashSet<String> = whole_partition_functions(
        project
            .borrow()
            .functions()
            .iter()
            .map(|record| &record.attached),
    );
    model_loop.get().run_sql_tests(
        host.bind(py),
        (&files, &scenarios),
        &settings,
        (arities, whole_partition),
    )
}

/// Attach singular, then resource-attached, audits; return views and override warnings.
pub(crate) fn attach_audits<'py>(
    project: &Bound<'py, NativeCompiledProject>,
    files: Vec<AuditFileRow>,
    settings: AuditSettings,
) -> PyResult<(Vec<NativeAuditInput>, Vec<Bound<'py, PyAny>>)> {
    let py: Python<'py> = project.py();
    let (model_loop, host) = project.borrow().attachment_loop(py)?;
    let model_loop: &Bound<'_, NativeModelLoop> = model_loop.bind(py).cast()?;
    let owners: Vec<AttachmentOwner<'py>> = attachment_owners(&project.borrow(), py)?;
    let (attached, shadows) =
        model_loop
            .get()
            .run_audits(host.bind(py), &files, owners, &settings)?;
    let count: usize = attached.len();
    let first: usize = project.borrow_mut().extend_audits(attached);
    Ok((
        (first..first + count)
            .map(|index| NativeAuditInput::new(project.clone().unbind(), index))
            .collect(),
        shadows,
    ))
}

/// Each schema-attached model, then each source and seed, with the audits attached to it.
fn attachment_owners<'py>(
    project: &NativeCompiledProject,
    py: Python<'py>,
) -> PyResult<Vec<AttachmentOwner<'py>>> {
    let mut owners: Vec<AttachmentOwner<'py>> = Vec::new();
    for record in project.inputs() {
        let (Some(schema), Some(fact)) = (&record.schema, project.input_schema_fact(record)) else {
            continue;
        };
        owners.push(model_owner(
            py,
            record.name.clone(),
            record.host.model_file.get().relative_text(py)?,
            (
                schema.host.audits.as_ref(),
                (schema.host.columns.as_ref(), &fact.columns),
            ),
        )?);
    }
    for source in project.sources() {
        let attached: &AttachedSource = source.attached();
        let path: String =
            native_field(attached.host.source_file.bind(py), RELATIVE_PATH_ATTRIBUTE)?
                .str()?
                .to_string();
        owners.push(resource_owner(
            SOURCE_OWNER,
            attached.name.clone(),
            (path.clone(), path),
            (
                attached.host.audits.bind(py).as_any(),
                attached.host.columns.bind(py).as_any(),
            ),
        )?);
    }
    for seed in project.seeds() {
        let SeedParts {
            name,
            seed_file,
            schema_entry,
            schema_file,
        } = seed.parts();
        let schema_entry: &Bound<'py, PyAny> = schema_entry.bind(py);
        owners.push(resource_owner(
            SEED_OWNER,
            name.to_owned(),
            (
                native_field(seed_file.bind(py), RELATIVE_PATH_ATTRIBUTE)?
                    .str()?
                    .to_string(),
                native_field(schema_file.bind(py), RELATIVE_PATH_ATTRIBUTE)?
                    .str()?
                    .to_string(),
            ),
            (
                &native_field(schema_entry, "audits")?,
                &native_field(schema_entry, "columns")?,
            ),
        )?);
    }
    Ok(owners)
}
