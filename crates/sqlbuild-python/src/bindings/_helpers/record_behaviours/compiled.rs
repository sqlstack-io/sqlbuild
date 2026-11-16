//! Normalization and derived views of the natively hosted compile output records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};

use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::entry_relation_key_row::entry_relation_key_row;

/// `CompilerDiagnostic` positions.
const DIAGNOSTIC_PHASE_FIELD: usize = 0;
const DIAGNOSTIC_SEVERITY_FIELD: usize = 1;
const DIAGNOSTIC_RESOURCE_TYPE_FIELD: usize = 4;
const DIAGNOSTIC_PATH_FIELD: usize = 7;
const DIAGNOSTIC_LINE_FIELD: usize = 8;
const DIAGNOSTIC_COLUMN_FIELD: usize = 9;
const DIAGNOSTIC_LOCATION_FIELD: usize = 10;
/// `CompiledSqlScenario` positions.
const SCENARIO_FILE_FIELD: usize = 2;
const SCENARIO_EXPECTED_MODELS_FIELD: usize = 11;
const SCENARIO_ASSERTION_TARGETS_FIELD: usize = 13;
const SCENARIO_TARGET_MODELS_FIELD: usize = 14;
const SCENARIO_SOURCE_PATH_FIELD: usize = 15;
const SCENARIO_OWNERSHIP_ROOT_FIELD: usize = 16;
/// `CompiledSqlTest` positions.
const TEST_FILE_FIELD: usize = 3;
const TEST_BLOCK_FIELD: usize = 4;
const TEST_SOURCE_PATH_FIELD: usize = 8;
const TEST_OWNERSHIP_ROOT_FIELD: usize = 9;
const TEST_BLOCK_INDEX_FIELD: usize = 10;
const TEST_EXPLICIT_NAME_FIELD: usize = 11;
const TEST_PARENT_NAME_FIELD: usize = 12;

/// The native string enum `class(value)`.
fn coerce_native(py: Python<'_>, value: &Py<PyAny>, class: RecordId) -> PyResult<Py<PyAny>> {
    Ok(new_record(py, class, (value.bind(py),), None)?.unbind())
}

/// `value` coerced to `CompiledResourceType` unless it is `None`.
fn coerce_resource_type(py: Python<'_>, value: &Py<PyAny>) -> PyResult<Py<PyAny>> {
    if value.is_none(py) {
        return Ok(value.clone_ref(py));
    }
    coerce_native(py, value, RecordId::CompiledResourceType)
}

/// `CompilerDiagnostic`: enum fields coerced, and its location and path, line and column aligned.
pub(crate) fn finish_compiler_diagnostic(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    values[DIAGNOSTIC_PHASE_FIELD] = coerce_native(
        py,
        &values[DIAGNOSTIC_PHASE_FIELD],
        RecordId::DiagnosticPhase,
    )?;
    values[DIAGNOSTIC_SEVERITY_FIELD] = coerce_native(
        py,
        &values[DIAGNOSTIC_SEVERITY_FIELD],
        RecordId::DiagnosticSeverity,
    )?;
    values[DIAGNOSTIC_RESOURCE_TYPE_FIELD] =
        coerce_resource_type(py, &values[DIAGNOSTIC_RESOURCE_TYPE_FIELD])?;
    let positioned: bool = [
        DIAGNOSTIC_PATH_FIELD,
        DIAGNOSTIC_LINE_FIELD,
        DIAGNOSTIC_COLUMN_FIELD,
    ]
    .iter()
    .all(|position| !values[*position].is_none(py));
    if values[DIAGNOSTIC_LOCATION_FIELD].is_none(py) && positioned {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("path", values[DIAGNOSTIC_PATH_FIELD].bind(py))?;
        keywords.set_item("line", values[DIAGNOSTIC_LINE_FIELD].bind(py))?;
        keywords.set_item("column", values[DIAGNOSTIC_COLUMN_FIELD].bind(py))?;
        let location: Bound<'_, PyAny> =
            new_record(py, RecordId::SourceLocation, (), Some(&keywords))?;
        values[DIAGNOSTIC_LOCATION_FIELD] = location.unbind();
    }
    let location: Bound<'_, PyAny> = values[DIAGNOSTIC_LOCATION_FIELD].bind(py).clone();
    if !location.is_none() {
        values[DIAGNOSTIC_PATH_FIELD] = native_field(&location, "path")?.unbind();
        values[DIAGNOSTIC_LINE_FIELD] = native_field(&location, "line")?.unbind();
        values[DIAGNOSTIC_COLUMN_FIELD] = native_field(&location, "column")?.unbind();
    }
    Ok(values)
}

/// `CompilerDiagnostic.is_error`: whether the diagnostic fails the command.
pub(crate) fn diagnostic_is_error(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let error: Bound<'_, PyAny> = enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?;
    native_field(instance, "severity")?
        .eq(error)?
        .into_py_any(py)
}

/// `CompileSqlReference`: its kind coerced to `SqlReferenceKind`.
pub(crate) fn finish_sql_reference(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    values[0] = coerce_native(py, &values[0], RecordId::SqlReferenceKind)?;
    Ok(values)
}

/// `CompiledLineageSourceFact` and `CompiledObjectKey`: the resource type coerced.
pub(crate) fn finish_resource_typed(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    values[0] = coerce_native(py, &values[0], RecordId::CompiledResourceType)?;
    Ok(values)
}

/// `value`, or `fallback` when `value` is `None`.
fn or_fallback(py: Python<'_>, value: &Py<PyAny>, fallback: Bound<'_, PyAny>) -> Py<PyAny> {
    if value.is_none(py) {
        return fallback.unbind();
    }
    value.clone_ref(py)
}

/// The distinct names of `first` then `second`, in first-seen order.
fn merged_names<'py>(
    first: &Bound<'py, PyAny>,
    second: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = first.py();
    let seen: Bound<'py, PyDict> = PyDict::new(py);
    for name in first.try_iter()?.chain(second.try_iter()?) {
        seen.set_item(name?, py.None())?;
    }
    Ok(PyTuple::new(py, seen.keys())?.into_any())
}

/// `CompiledSqlScenario`: source path, ownership and target models taken from its file.
pub(crate) fn finish_sql_scenario(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let scenario_file: Bound<'_, PyAny> = values[SCENARIO_FILE_FIELD].bind(py).clone();
    values[SCENARIO_SOURCE_PATH_FIELD] = or_fallback(
        py,
        &values[SCENARIO_SOURCE_PATH_FIELD],
        native_field(&scenario_file, "relative_path")?,
    );
    values[SCENARIO_OWNERSHIP_ROOT_FIELD] = or_fallback(
        py,
        &values[SCENARIO_OWNERSHIP_ROOT_FIELD],
        native_field(&scenario_file, "ownership_root")?,
    );
    if !values[SCENARIO_TARGET_MODELS_FIELD].bind(py).is_truthy()? {
        let targets: Bound<'_, PyAny> = merged_names(
            values[SCENARIO_EXPECTED_MODELS_FIELD].bind(py),
            values[SCENARIO_ASSERTION_TARGETS_FIELD].bind(py),
        )?;
        values[SCENARIO_TARGET_MODELS_FIELD] = targets.unbind();
    }
    Ok(values)
}

/// `CompiledSqlTest`: source path, ownership, block index and names taken from its file and block.
pub(crate) fn finish_sql_test(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let test_file: Bound<'_, PyAny> = values[TEST_FILE_FIELD].bind(py).clone();
    let test_block: Bound<'_, PyAny> = values[TEST_BLOCK_FIELD].bind(py).clone();
    values[TEST_SOURCE_PATH_FIELD] = or_fallback(
        py,
        &values[TEST_SOURCE_PATH_FIELD],
        native_field(&test_file, "relative_path")?,
    );
    values[TEST_OWNERSHIP_ROOT_FIELD] = or_fallback(
        py,
        &values[TEST_OWNERSHIP_ROOT_FIELD],
        native_field(&test_file, "ownership_root")?,
    );
    values[TEST_BLOCK_INDEX_FIELD] = or_fallback(
        py,
        &values[TEST_BLOCK_INDEX_FIELD],
        native_field(&test_block, "test_index")?,
    );
    let block_name: Bound<'_, PyAny> = native_field(&test_block, "name")?;
    if !block_name.is_none() {
        values[TEST_EXPLICIT_NAME_FIELD] =
            or_fallback(py, &values[TEST_EXPLICIT_NAME_FIELD], block_name.clone());
    }
    if values[TEST_PARENT_NAME_FIELD].is_none(py) {
        let parent: Bound<'_, PyAny> = if block_name.is_truthy()? {
            block_name
        } else {
            native_field(&native_field(&test_file, "relative_path")?, "stem")?
        };
        values[TEST_PARENT_NAME_FIELD] = parent.unbind();
    }
    Ok(values)
}

/// `(kind, name, package, call argument count)` of each reference in `references`.
fn reference_rows(references: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = references.py();
    let rows: Bound<'_, PyList> = PyList::empty(py);
    for reference in references.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        rows.append((
            native_field(&reference, "ref_kind")?.str()?,
            native_field(&reference, "ref_name")?,
            native_field(&reference, "ref_package")?,
            native_field(&reference, "call_argument_count")?,
        ))?;
    }
    Ok(rows.into_any().unbind())
}

/// `CompileModelInput.reference_rows` and `CompiledModel.reference_rows`.
pub(crate) fn record_reference_rows(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    reference_rows(&native_field(instance, "references")?)
}

/// The declared key facts of model `name` with `instance`'s config and schema entry.
fn model_relation_key_row(instance: &Bound<'_, PyAny>, name: String) -> PyResult<Py<PyAny>> {
    let values: Bound<'_, PyAny> = native_field(&native_field(instance, "config")?, "values")?;
    entry_relation_key_row(
        "ref",
        name,
        values.cast::<PyDict>()?,
        &native_field(instance, "schema_entry")?,
    )?
    .into_py_any(instance.py())
}

/// `CompileModelInput.relation_key_row`: keyed by the model file's stem.
pub(crate) fn input_relation_key_row(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let name: String = native_field(
        &native_field(&native_field(instance, "model_file")?, "file_path")?,
        "stem",
    )?
    .extract()?;
    model_relation_key_row(instance, name)
}

/// `CompiledModel.relation_key_row`: keyed by the model name.
pub(crate) fn model_relation_key_row_property(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let name: String = native_field(instance, "name")?.extract()?;
    model_relation_key_row(instance, name)
}

/// `CompileModelInput.attached_schema_name`: the attached schema entry's name, or `None`.
pub(crate) fn input_attached_schema_name(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let entry: Bound<'_, PyAny> = native_field(instance, "schema_entry")?;
    if entry.is_none() {
        return Ok(entry.unbind());
    }
    Ok(native_field(&entry, "name")?.unbind())
}

/// `(resource type, name)` of one compiled object key.
fn key_row<'py>(key: &Bound<'py, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(key, "resource_type")?.str()?.extract()?,
        native_field(key, "name")?.extract()?,
    ))
}

/// `(resource type, name)` of each compiled object key.
fn key_rows(keys: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    keys.try_iter()?.map(|key| key_row(&key?)).collect()
}

/// `CompiledModel.key_row`.
pub(crate) fn model_key_row(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    key_row(&native_field(instance, "key")?)?.into_py_any(instance.py())
}

/// `CompiledModel.dep_rows`.
pub(crate) fn model_dep_rows(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    key_rows(&native_field(instance, "deps")?)?.into_py_any(instance.py())
}

/// `CompiledModel.destination_row`: `(database, schema, name)` of the destination.
pub(crate) fn model_destination_row(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let destination: Bound<'_, PyAny> = native_field(instance, "destination")?;
    (
        native_field(&destination, "database")?,
        native_field(&destination, "schema")?,
        native_field(&destination, "name")?,
    )
        .into_py_any(instance.py())
}

/// `CompiledModel.inferred_rows`: each inferred column as `(name, type, nullability)`.
pub(crate) fn model_inferred_rows(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let columns: Bound<'_, PyAny> = native_field(instance, "inferred_columns")?;
    if columns.is_none() {
        return Ok(columns.unbind());
    }
    let rows: Bound<'_, PyList> = PyList::empty(instance.py());
    for column in columns.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        rows.append((
            native_field(&column, "name")?,
            native_field(&column, "type")?,
            native_field(&column, "nullability")?.str()?,
        ))?;
    }
    Ok(rows.into_any().unbind())
}

/// `CompiledModel.schema_column_rows`: each declared schema column as `(name, type)`.
pub(crate) fn model_schema_column_rows(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let entry: Bound<'_, PyAny> = native_field(instance, "schema_entry")?;
    if entry.is_none() {
        return Ok(entry.unbind());
    }
    let rows: Bound<'_, PyList> = PyList::empty(instance.py());
    for column in native_field(&entry, "columns")?.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        rows.append((
            native_field(&column, "name")?,
            native_field(&column, "type")?,
        ))?;
    }
    Ok(rows.into_any().unbind())
}

/// `CompiledModel.has_dynamic_columns`.
pub(crate) fn model_has_dynamic_columns(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let entry: Bound<'_, PyAny> = native_field(instance, "schema_entry")?;
    let dynamic: bool = !entry.is_none() && native_field(&entry, "dynamic_columns")?.is_truthy()?;
    dynamic.into_py_any(instance.py())
}

/// `CompiledModel.output_column_count`: inferred, else declared, columns, else zero.
pub(crate) fn model_output_column_count(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let inferred: Bound<'_, PyAny> = native_field(instance, "inferred_columns")?;
    let entry: Bound<'_, PyAny> = native_field(instance, "schema_entry")?;
    let count: usize = if !inferred.is_none() {
        inferred.len()?
    } else if !entry.is_none() {
        native_field(&entry, "columns")?.len()?
    } else {
        0
    };
    count.into_py_any(instance.py())
}

/// `ModelInputBuildContext.declaration_expansion`: the public declarations for non-model SQL.
pub(crate) fn build_context_declaration_expansion(
    instance: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let declarations: Bound<'_, PyDict> = PyDict::new(py);
    declarations.set_item("enums", native_field(instance, "public_enums")?)?;
    declarations.set_item("constants", native_field(instance, "public_constants")?)?;
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item(
        "declarations",
        new_record(
            py,
            RecordId::DeclarationResolutionContext,
            (),
            Some(&declarations),
        )?,
    )?;
    keywords.set_item("value_renderer", native_field(instance, "value_renderer")?)?;
    keywords.set_item(
        "collection_rendering",
        native_field(instance, "collection_rendering")?,
    )?;
    keywords.set_item("resolver", native_field(instance, "declaration_resolver")?)?;
    Ok(new_record(
        py,
        RecordId::DeclarationExpansionContext,
        (),
        Some(&keywords),
    )?
    .unbind())
}

/// A `CompileInputError` carrying `message`.
pub(crate) fn compile_input_error(py: Python<'_>, message: &str) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::CompileInput)?.call1((message,))?,
    ))
}
