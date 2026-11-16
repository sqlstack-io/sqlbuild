//! Where the compile's report and output stages leave image facts or take a delta's injection.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyStringMethods, PyTuple, PyTupleMethods};
use sqlbuild_core::cli_text::models::ModelLine;

use crate::bindings::_helpers::compile_command::report::diagnostic_item;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::main::delta::guarded::guarded;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_outputs::main::plain_json::plain_json;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The graph key kind of a model.
const MODEL_KIND: &str = "model";

/// The text report's model lines, captured or, in a delta, patched for the rerun models.
pub(crate) fn text_model_lines(models: Vec<ModelLine>) -> Vec<ModelLine> {
    let models: Vec<ModelLine> = match injection() {
        None => models,
        Some(injection) => {
            let mut fresh: HashMap<String, ModelLine> = models
                .into_iter()
                .map(|line| (line.name.clone(), line))
                .collect();
            injection
                .text_models
                .iter()
                .map(|(name, failed, column_count)| {
                    fresh.remove(name).unwrap_or_else(|| ModelLine {
                        name: name.clone(),
                        failed: *failed,
                        column_count: *column_count,
                    })
                })
                .collect()
        }
    };
    capture(|capture| {
        capture.text_models = Some(
            models
                .iter()
                .map(|line| (line.name.clone(), line.failed, line.column_count))
                .collect(),
        );
    });
    models
}

/// Record which models each SQL test's compiled artifact is keyed by.
fn record_test_chains_inner(tests: &Bound<'_, PyAny>, chains: &Bound<'_, PyAny>) -> PyResult<()> {
    if !capturing() {
        return Ok(());
    }
    let mut captured: Vec<CapturedTest> = Vec::new();
    for test in tests.try_iter()? {
        let test: Bound<'_, PyAny> = test?;
        let key: Bound<'_, PyAny> = native_field(&test, "key")?;
        let chain: Vec<String> = match chains.get_item(&key) {
            Ok(chain) => chain.extract()?,
            Err(_) => Vec::new(),
        };
        captured.push(CapturedTest {
            key: native_field(&test, "name")?.str()?.to_str()?.to_owned(),
            relative_path: native_field(&native_field(&test, "test_file")?, "relative_path")?
                .str()?
                .to_str()?
                .replace('\\', "/"),
            chain,
        });
    }
    capture(|capture| capture.test_chains = captured);
    Ok(())
}

/// Record the compiled models with their references, and the report's diagnostics.
fn record_compile_facts_inner(
    graph: &Bound<'_, PyAny>,
    diagnostics: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    if !capturing() {
        return Ok(());
    }
    let native: Bound<'_, PyAny> = native_field(graph, "native")?;
    let graph_native: &NativeProjectGraph = native.cast::<NativeProjectGraph>()?.get();
    let mut models: Vec<CapturedModel> = Vec::new();
    for model in native_field(&native_field(graph, "project")?, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        let upstream: Vec<(String, String)> =
            graph_native.upstream((MODEL_KIND.to_owned(), name.clone()));
        models.push(CapturedModel {
            name,
            relative_path: native_field(&model, "relative_path")?
                .str()?
                .to_str()?
                .replace('\\', "/"),
            upstream,
        });
    }
    let mut captured: Vec<CapturedDiagnostic> = Vec::new();
    for diagnostic in diagnostics.iter() {
        captured.push(captured_diagnostic(&diagnostic)?);
    }
    let test_count: usize = native_field(&native_field(graph, "project")?, "sql_tests")?.len()?;
    capture(|capture| {
        capture.models = models;
        capture.diagnostics = captured;
        if capture.test_chains.len() != test_count {
            capture
                .unsupported
                .get_or_insert_with(|| "the SQL tests' model chains were not recorded".to_owned());
        }
    });
    Ok(())
}

/// One diagnostic as its report JSON, with the resource and paths it names.
pub(crate) fn captured_diagnostic(diagnostic: &Bound<'_, PyAny>) -> PyResult<CapturedDiagnostic> {
    let item = diagnostic_item(diagnostic)?;
    let json: String = plain_json(item.as_any())?;
    let value: serde_json::Value = match serde_json::from_str(&json) {
        Ok(value) => value,
        Err(_malformed) => serde_json::Value::Null,
    };
    let text = |value: Option<&serde_json::Value>| -> Option<String> {
        value.and_then(serde_json::Value::as_str).map(str::to_owned)
    };
    let mut paths: Vec<String> = Vec::new();
    paths.extend(text(value.get("path")));
    paths.extend(text(
        value
            .get("location")
            .and_then(|location| location.get("path")),
    ));
    Ok(CapturedDiagnostic {
        json,
        resource_name: text(value.get("resource_name")),
        paths: paths
            .into_iter()
            .map(|path| path.replace('\\', "/"))
            .collect(),
        error: value.get("severity").and_then(serde_json::Value::as_str) == Some("error"),
    })
}

/// Record which models each SQL test's compiled artifact is keyed by.
pub(crate) fn record_test_chains(
    tests: &Bound<'_, PyAny>,
    chains: &Bound<'_, PyAny>,
) -> PyResult<()> {
    guarded(|| record_test_chains_inner(tests, chains))
}

/// Record the compiled models with their references, and the report's diagnostics.
pub(crate) fn record_compile_facts(
    graph: &Bound<'_, PyAny>,
    diagnostics: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    guarded(|| record_compile_facts_inner(graph, diagnostics))
}

/// In a delta, add the text of files that stored diagnostics point into but were not rerun.
pub(crate) fn complete_source_texts(
    project: &Bound<'_, PyAny>,
    diagnostics: &[Bound<'_, PyAny>],
    texts: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let Some(injection) = injection() else {
        return Ok(());
    };
    let _ = project;
    let project_dir: &std::path::Path = &injection.project_dir;
    let mut locations: Vec<Bound<'_, PyAny>> = Vec::new();
    for diagnostic in diagnostics {
        locations.push(native_field(diagnostic, "location")?);
        for item in native_field(diagnostic, "related_locations")?.try_iter()? {
            locations.push(native_field(&item?, "location")?);
        }
    }
    for location in locations.into_iter().filter(|location| !location.is_none()) {
        let path: Bound<'_, PyAny> = native_field(&location, "path")?;
        if texts.contains(&path)? {
            continue;
        }
        let relative: String = path.str()?.to_str()?.replace('\\', "/");
        if !injection.source_paths.contains(&relative) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(project_dir.join(&relative)) {
            texts.set_item(&path, text)?;
        }
    }
    Ok(())
}
