//! Key rows and compiled expansions lint reads natively from compile inputs and compiled projects.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::rules::_helpers::lint::lint_pipeline::CompiledExpansion;
use crate::rules::_helpers::lint::lint_pipeline::CompiledSource;
use crate::rules::_helpers::lint::lint_pipeline::span_pass;
use sqlbuild_project::models::ExpansionRecord;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::render::types::SpanRow;
use sqlbuild_python_project::compiled_views::models::NativeModelInput;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::compiled_views::types::ModelInputRecord;

const FILE_PATH_ATTRIBUTE: &str = "file_path";

/// The model file path of a native input record.
fn record_path(record: &ModelInputRecord) -> PathBuf {
    record.host.model_file.get().file_path_buf()
}

/// The record's shared texts with the given passes, read only when their file is linted.
fn expansion(expansion: &ExpansionRecord, passes: &[&Vec<SpanRow>]) -> CompiledSource {
    CompiledSource::Shared {
        authored_sql: Arc::clone(&expansion.authored_sql),
        expanded_sql: Arc::clone(&expansion.expanded_sql),
        passes: passes.iter().map(|rows| span_pass(rows)).collect(),
    }
}

/// A model input's whole compile expansion, variables included, keyed by its file path.
pub(crate) fn model_lint_expansion(
    model: &Bound<'_, PyAny>,
) -> PyResult<Option<(PathBuf, CompiledSource)>> {
    let py: Python<'_> = model.py();
    if let Ok(input) = model.downcast::<NativeModelInput>() {
        return input.get().read(py, |record, _| {
            let whole: &ExpansionRecord = &record.expansion;
            Ok(Some((
                record_path(record),
                expansion(
                    whole,
                    &[
                        &whole.interpolation_spans,
                        &whole.declaration_spans,
                        &whole.macro_spans,
                    ],
                ),
            )))
        });
    }
    let mut record: Bound<'_, PyAny> = native_field(model, "lint_expansion")?;
    if !record.is_truthy()? {
        record = native_field(model, "sql_expansion")?;
    }
    if record.is_none() {
        return Ok(None);
    }
    let path: PathBuf = native_field(model, "model_file")?
        .getattr(FILE_PATH_ATTRIBUTE)?
        .extract()?;
    Ok(Some((
        path,
        CompiledSource::Owned(CompiledExpansion::from_record(&record)?),
    )))
}

/// A compiled project's authored expansions: models whose interpolation left the query unchanged.
pub(crate) fn project_sql_expansions(
    project: &Bound<'_, PyAny>,
) -> PyResult<HashMap<PathBuf, CompiledSource>> {
    let py: Python<'_> = project.py();
    let mut expansions: HashMap<PathBuf, CompiledSource> = HashMap::new();
    if let Ok(view) = project.downcast::<NativeProjectView>() {
        let owner = view.get().owner().bind(py).borrow();
        for record in owner.inputs() {
            let authored: &ExpansionRecord = &record.expansion;
            if authored.authored {
                expansions.insert(
                    record_path(record),
                    expansion(
                        authored,
                        &[&authored.declaration_spans, &authored.macro_spans],
                    ),
                );
            }
        }
        return Ok(expansions);
    }
    crate::rules::_helpers::lint::run_lint::compiled_expansion_records(&native_field(
        project,
        "sql_expansions",
    )?)
}
