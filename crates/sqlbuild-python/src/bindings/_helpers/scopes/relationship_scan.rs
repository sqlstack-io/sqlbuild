//! Relationship facts of SQL tests and scenarios, reusing and storing whole files' scans.

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyResult, Python};
use sqlbuild_scopes::relationship_names::main::relationship_facts::relationship_facts;
use sqlbuild_scopes::relationship_names::main::stored_expected_names::stored_expected_names;
use sqlbuild_scopes::relationship_names::models::{
    RelationshipScan, ScenarioInput, TestBlockInput, TestFileInput,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::cache::models::SqlTestScanStore;
use sqlbuild_python_base::types::CompilerDetach;

/// A test file's relative path and each block's name, SQL and mode.
pub(crate) type TestFileRow = (String, Vec<(String, String, String)>);
/// A scenario's name, relative path and SQL.
pub(crate) type ScenarioRow = (String, String, String);

/// The lexical rules the scans follow and the store, with its key prefix, they reuse.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct RelationshipScanSettings {
    syntax: LexicalSyntaxInput,
    /// The shared scan store, or `None` when this compile keeps no cache.
    store: Option<Py<SqlTestScanStore>>,
    /// The scan algorithm and lexical rules keying stored files.
    scan_key: (String, String),
}

/// One file's store key: the key prefix, its path, then each block's SQL and mode.
fn key_parts(settings: &RelationshipScanSettings, file: &TestFileRow) -> Vec<String> {
    let (relative_path, blocks) = file;
    let mut parts: Vec<String> = vec![
        settings.scan_key.0.clone(),
        settings.scan_key.1.clone(),
        relative_path.clone(),
    ];
    for (_, sql, mode) in blocks {
        parts.push(sql.clone());
        parts.push(mode.clone());
    }
    parts
}

/// Every fact and fault, after storing each file that scanned without a fault.
pub(crate) fn scanned_relationships(
    py: Python<'_>,
    files: &[TestFileRow],
    scenarios: &[ScenarioRow],
    settings: RelationshipScanSettings,
) -> PyResult<RelationshipScan> {
    let store: Option<Bound<'_, SqlTestScanStore>> =
        settings.store.as_ref().map(|store| store.bind(py).clone());
    let mut stored: Vec<Option<Vec<Vec<String>>>> = Vec::with_capacity(files.len());
    for file in files {
        stored.push(match &store {
            Some(store) => store.get().decoded(key_parts(&settings, file), |value| {
                stored_expected_names(value, file.1.len())
            })?,
            None => None,
        });
    }
    let mut inputs: Vec<TestFileInput<'_>> = Vec::with_capacity(files.len());
    for ((relative_path, blocks), stored) in files.iter().zip(stored) {
        let mut block_inputs: Vec<TestBlockInput<'_>> = Vec::with_capacity(blocks.len());
        for (name, sql, mode) in blocks {
            block_inputs.push(TestBlockInput { name, sql, mode });
        }
        inputs.push(TestFileInput {
            relative_path,
            blocks: block_inputs,
            stored,
        });
    }
    let scenario_inputs: Vec<ScenarioInput<'_>> = scenarios
        .iter()
        .map(|(name, relative_path, sql)| ScenarioInput {
            name,
            relative_path,
            sql,
        })
        .collect();
    let syntax: LexicalSyntax = settings.syntax.clone().into();
    let scan: RelationshipScan = py
        .compiler_detach(|| Ok(relationship_facts(&inputs, &scenario_inputs, &syntax)))
        .map_err(compiler_error)?;
    if let Some(store) = &store {
        for (position, names) in &scan.scanned_files {
            let value: Vec<u8> = serde_json::to_vec(names)
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            store
                .get()
                .stored(key_parts(&settings, &files[*position]), value)?;
        }
    }
    Ok(scan)
}
