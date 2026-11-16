//! Compiled records and views: projecting native views as records, and recording later stages.

use crate::compiled_views::main::project::declaration_rows::declaration_rows;
use crate::compiled_views::main::project::plain_typed_value::plain_typed_value;
use crate::compiled_views::main::project::record_model_analyses::record_model_analyses;
use crate::compiled_views::main::project::replace::replace;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple, PyType};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;

use crate::compiled_project::main::entry_schema_fact::entry_schema_fact;
use crate::compiled_project::main::model_facts::model_facts;
use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeProjectView;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::record_spec::record_spec;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) const DATACLASSES_MODULE: &str = "dataclasses";
pub(crate) const DECIMAL_KIND: &str = "decimal";
pub(crate) const COLLECTION_KINDS: [&str; 2] = ["list", "set"];
pub(crate) const SCALAR_KINDS: [&str; 5] = ["string", "integer", "boolean", "float", "null"];

/// `value` when it already is a `class` record, else `class(**{field: getattr(value, field)})`.
pub(crate) fn record<'py>(
    value: &Bound<'py, PyAny>,
    class: RecordId,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let class: Bound<'py, PyAny> = record_class(py, class)?;
    if value.is_instance(&class)? {
        return Ok(value.clone());
    }
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    for field in record_spec(class.cast::<PyType>()?)?.fields {
        fields.set_item(field.name, native_field(value, field.name)?)?;
    }
    class.call((), Some(&fields))
}

/// Each item of `fields[name]` as a `class` record.
fn records_of<'py>(fields: &Bound<'py, PyDict>, (name, class): (&str, RecordId)) -> PyResult<()> {
    let py: Python<'py> = fields.py();
    let items: Vec<Bound<'py, PyAny>> = fields
        .as_any()
        .get_item(name)?
        .try_iter()?
        .map(|item| record(&item?, class))
        .collect::<PyResult<_>>()?;
    fields.set_item(name, PyTuple::new(py, items)?)
}

/// The project as a `CompiledProject`, projected from its owner when it is a native view.
#[pyfunction]
#[pyo3(signature = (project, *, include_sql_expansions=true))]
pub(crate) fn compiled_project_record<'py>(
    project: &Bound<'py, PyAny>,
    include_sql_expansions: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let class: Bound<'py, PyAny> = record_class(py, RecordId::CompiledProject)?;
    if project.is_instance(&class)? {
        if include_sql_expansions {
            return Ok(project.clone());
        }
        return replace(project, &[("sql_expansions", PyDict::new(py).into_any())]);
    }
    let view: PyRef<'py, NativeProjectView> = project.extract()?;
    let fields: Bound<'py, PyDict> = view.fields(py, include_sql_expansions)?;
    drop(view);
    for pair in [
        ("seeds", RecordId::CompiledSeed),
        ("sources", RecordId::CompiledSource),
        ("functions", RecordId::CompiledFunction),
        ("audits", RecordId::CompiledAudit),
    ] {
        records_of(&fields, pair)?;
    }
    new_record(py, RecordId::CompiledProject, (), Some(&fields))
}

/// `replace(value, **changes)` for a record, else the native view's `with_changes(**changes)`.
pub(crate) fn with_changes<'py>(
    value: &Bound<'py, PyAny>,
    class: RecordId,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let changes: Bound<'py, PyDict> = changes.unwrap_or_else(|| PyDict::new(py));
    if value.is_instance(&record_class(py, class)?)? {
        return imported(py, DATACLASSES_MODULE, "replace")?.call((value,), Some(&changes));
    }
    value.call_method("with_changes", (), Some(&changes))
}

/// An enum declaration as the native project row.
pub(crate) fn enum_row<'py>(declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = declaration.py();
    let members: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> =
        native_field(declaration, "members")?
            .try_iter()?
            .map(|member| {
                let member: Bound<'py, PyAny> = member?;
                Ok((
                    native_field(&member, "name")?,
                    native_field(&member, "value")?,
                ))
            })
            .collect::<PyResult<_>>()?;
    PyTuple::new(
        py,
        [
            native_field(declaration, "name")?,
            native_field(declaration, "relative_path")?.call_method0("as_posix")?,
            members.into_bound_py_any(py)?,
            py.None().into_bound(py),
            py.None().into_bound(py),
            py.None().into_bound(py),
        ],
    )
}

/// A constant declaration as the native project row, its value made JSON-safe.
pub(crate) fn constant_row<'py>(declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = declaration.py();
    let render_as: Bound<'py, PyAny> = native_field(declaration, "render_as")?;
    PyTuple::new(
        py,
        [
            native_field(declaration, "name")?,
            native_field(declaration, "relative_path")?.call_method0("as_posix")?,
            PyList::empty(py).into_any(),
            plain_typed_value(&native_field(declaration, "value")?)?,
            native_field(declaration, "logical_type")?.getattr("display_name")?,
            if render_as.is_none() {
                render_as
            } else {
                native_field(&render_as, "value")?
            },
        ],
    )
}

/// `(name, type)` of each dynamic family; `casefold` keys the family by its folded name.
pub(crate) fn family_types<'py>(
    families: &Bound<'py, PyAny>,
    casefold: bool,
) -> PyResult<Vec<(String, Option<String>)>> {
    families
        .try_iter()?
        .map(|family| {
            let family: Bound<'py, PyAny> = family?;
            let name: Bound<'py, PyAny> = native_field(&family, "name")?;
            let name: String = if casefold {
                name.call_method0("casefold")?.extract()?
            } else {
                name.extract()?
            };
            Ok((name, native_field(&family, "inferred_type")?.extract()?))
        })
        .collect()
}

/// A native project for a `CompiledProject` built without the compile-input stage.
pub(crate) fn hand_built_project<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeCompiledProject>> {
    let py: Python<'py> = project.py();
    let native: Bound<'py, NativeCompiledProject> =
        Bound::new(py, NativeCompiledProject::default())?;
    let models: Bound<'py, PyAny> = native_field(project, "models")?;
    for model in models.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let mut facts: CompiledModelFacts = model_facts(model_row(&model)?.as_any().extract()?)?;
        facts.schema = entry_schema_fact(
            &native_field(&model, "schema_entry")?,
            &native_field(&model, "relative_path")?,
        )?;
        native.borrow_mut().retain(facts);
    }
    let _ = record_model_analyses(&native, &models)?;
    Ok(native)
}

fn model_row<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let relative_path: Bound<'py, PyAny> = native_field(model, "relative_path")?;
    let config: Bound<'py, PyAny> = native_field(model, "config")?;
    let references: Vec<Bound<'py, PyTuple>> = native_field(model, "references")?
        .try_iter()?
        .map(|reference| {
            let reference: Bound<'py, PyAny> = reference?;
            PyTuple::new(
                py,
                [
                    native_field(&reference, "ref_kind")?.str()?.into_any(),
                    native_field(&reference, "ref_name")?,
                    native_field(&reference, "ref_package")?,
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    let (enums, constants) = declaration_rows(
        &native_field(model, "enum_declarations")?,
        &native_field(model, "constant_declarations")?,
    )?;
    PyTuple::new(
        py,
        [
            PyTuple::new(
                py,
                [
                    native_field(model, "name")?,
                    relative_path.call_method0("as_posix")?,
                    native_field(model, "query_sql")?,
                    native_field(model, "authored_query_sql")?,
                    native_field(model, "authored_sql")?,
                ],
            )?
            .into_any(),
            PyTuple::new(
                py,
                [
                    native_field(&config, "values")?,
                    PyList::new(
                        py,
                        native_field(&config, "model_header_keys")?
                            .try_iter()?
                            .collect::<PyResult<Vec<_>>>()?,
                    )?
                    .into_any(),
                    native_field(&config, "layer_schema")?,
                ],
            )?
            .into_any(),
            PyList::new(py, references)?.into_any(),
            PyList::new(
                py,
                native_field(model, "enum_columns")?
                    .try_iter()?
                    .collect::<PyResult<Vec<_>>>()?,
            )?
            .into_any(),
            PyTuple::new(py, [enums.into_any(), constants.into_any()])?.into_any(),
        ],
    )
}
