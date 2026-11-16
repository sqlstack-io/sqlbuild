//! Where the compile's report and output stages leave image facts or take a delta's injection.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyStringMethods, PyTuple, PyTupleMethods};

use crate::compile_command::main::invocation::diagnostic_item::diagnostic_item;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_outputs::main::plain_json::plain_json;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::graph::models::NativeProjectGraph;

/// The graph key kind of a model.
const MODEL_KIND: &str = "model";

/// Record which models each SQL test's compiled artifact is keyed by.
pub(crate) fn record_test_chains_inner(
    tests: &Bound<'_, PyAny>,
    chains: &Bound<'_, PyAny>,
) -> PyResult<()> {
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
pub(crate) fn record_compile_facts_inner(
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
