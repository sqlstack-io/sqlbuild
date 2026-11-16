//! The process-local declaration scope of one compile invocation.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

use crate::bindings::_helpers::render::declaration_values::declaration_scope_resolver;
use crate::bindings::_helpers::scopes::scope_index::open_native_scope_index;
use crate::bindings::_helpers::scopes::scope_lookup::build_scope_lookup;
use crate::bindings::_helpers::scopes::scope_validation::validate_scope_index;
use crate::bindings::_helpers::sql_test_glue::authored_ctes::validate_authored_sql_test_ctes;
use crate::bindings::_helpers::sql_test_glue::constants::SQL_TEST_EXPECTED_MODELS_SCAN_ALGORITHM;
use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::cache::models::SqlTestScanCache;
use sqlbuild_python_base::compile_driver::constants::DECLARATION_SCOPES;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

fn compile_error(py: Python<'_>, message: Bound<'_, PyAny>) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::CompileInput)?.call1((message,))?,
    ))
}

/// The declaration scope of `index`, resolved for `loaded_macros`.
fn scope_build<'py>(
    discovered: &Bound<'py, PyAny>,
    loaded_macros: &Bound<'py, PyAny>,
    (index, lookup, scan_cache): (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let resolver: Bound<'py, PyAny> =
        declaration_scope_resolver(discovered, &index, (loaded_macros, &lookup))?;
    new_record_with(
        py,
        RecordId::DeclarationScopeBuild,
        &[
            ("loaded_macros", loaded_macros.clone()),
            ("index", index),
            ("resolver", resolver),
            ("sql_test_scans", scan_cache),
        ],
    )
}

/// Build one canonical index and validate it before SQL expansion.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, loaded_macros, sql_lexical_syntax, compile_cache_dir=None))]
pub(crate) fn build_declaration_scope<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    loaded_macros: &Bound<'py, PyAny>,
    sql_lexical_syntax: &Bound<'py, PyAny>,
    compile_cache_dir: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered_inputs.py();
    count_stage(py, DECLARATION_SCOPES, 1)?;
    let scan_cache: Bound<'py, PyAny> = pyo3::Bound::new(
        py,
        SqlTestScanCache::new(
            compile_cache_dir
                .unwrap_or_else(|| py.None().into_bound(py))
                .extract::<Option<std::path::PathBuf>>()?,
        ),
    )
    .map(pyo3::Bound::into_any)?;
    let test_files: Bound<'py, PyAny> = native_field(discovered_inputs, "test_files")?;
    let scenario_files: Bound<'py, PyAny> = native_field(discovered_inputs, "scenario_files")?;
    validate_authored_sql_test_ctes(
        py,
        test_files.clone(),
        scenario_files.clone(),
        native_field(sql_lexical_syntax, "native_mapping")?.extract::<LexicalSyntaxInput>()?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let native: Bound<'py, PyAny> = Bound::new(
        py,
        open_native_scope_index(py, discovered_inputs, loaded_macros, None)?,
    )?
    .into_any();
    let validated: Result<(), PyErr> = validate_scope_index(&native_field(&native, "index")?);
    if let Err(error) = validated {
        if !error.is_instance(py, &error_class(py, ErrorId::ScopeValidation)?) {
            return Err(error);
        }
        let raised: PyErr = compile_error(py, error.value(py).str()?.into_any())?;
        raised.set_cause(py, Some(error));
        return Err(raised);
    }
    if native_field(&native, "has_scoped_relationship_declarations")?.is_truthy()?
        && (test_files.is_truthy()? || scenario_files.is_truthy()?)
    {
        let faults: Bound<'py, PyAny> = call_with(
            &native_field(&native, "grant_relationships")?,
            &[
                ("discovered_inputs", discovered_inputs.clone()),
                ("syntax", sql_lexical_syntax.clone()),
                (
                    "scan_store",
                    PyTuple::new(
                        py,
                        [
                            scan_cache.call_method0("opened")?,
                            pyo3::types::PyString::new(py, SQL_TEST_EXPECTED_MODELS_SCAN_ALGORITHM)
                                .into_any(),
                        ],
                    )?
                    .into_any(),
                ),
            ],
        )?;
        if faults.is_truthy()? {
            return Err(compile_error(py, faults.get_item(0)?.get_item(1)?)?);
        }
    }
    let index: Bound<'py, PyAny> = native.call_method0("index_with_relationships")?;
    let lookup: Bound<'py, PyAny> = build_scope_lookup(py, &index)?.into_bound(py);
    scope_build(
        discovered_inputs,
        loaded_macros,
        (index, lookup, scan_cache),
    )
}

/// Each loaded macro's metadata that the built index recorded.
fn indexed_macro_metadata<'py>(loaded_macros: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = loaded_macros.py();
    let code_type: Bound<'py, PyAny> = imported(py, "types", "CodeType")?;
    let signature: Bound<'py, PyAny> = imported(py, "inspect", "signature")?;
    let mut rows: Vec<Bound<'py, PyTuple>> = Vec::new();
    for pair in mapping_items(loaded_macros)?.try_iter()? {
        let (key, loaded): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        let function: Bound<'py, PyAny> = native_field(&loaded, "function")?;
        let code: Bound<'py, PyAny> =
            imported(py, "builtins", "getattr")?.call1((&function, "__code__", py.None()))?;
        let first_line: Bound<'py, PyAny> = if code.is_instance(&code_type)? {
            native_field(&code, "co_firstlineno")?
        } else {
            py.None().into_bound(py)
        };
        let parameters: Bound<'py, PyTuple> = PyTuple::new(
            py,
            native_field(&signature.call1((&function,))?, "parameters")?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?,
        )?;
        rows.push(PyTuple::new(
            py,
            [
                key,
                native_field(&loaded, "name")?,
                native_field(&loaded, "relative_path")?,
                native_field(&loaded, "raw_source")?,
                native_field(&loaded, "dependencies")?,
                first_line,
                parameters.into_any(),
            ],
        )?);
    }
    PyTuple::new(py, rows)
}

/// Pair a built index with private macro instances, or return None if their metadata differs.
#[pyfunction]
#[pyo3(signature = (*, scope, discovered_inputs, loaded_macros))]
pub(crate) fn rebind_declaration_scope<'py>(
    scope: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    loaded_macros: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = scope.py();
    let current: Bound<'py, PyTuple> = indexed_macro_metadata(loaded_macros)?;
    let indexed: Bound<'py, PyTuple> =
        indexed_macro_metadata(&native_field(scope, "loaded_macros")?)?;
    if current.as_any().ne(indexed.as_any())? {
        return Ok(py.None().into_bound(py));
    }
    scope_build(
        discovered_inputs,
        loaded_macros,
        (
            native_field(scope, "index")?,
            native_field(scope, "resolver")?.getattr("lookup")?,
            native_field(scope, "sql_test_scans")?,
        ),
    )
}
