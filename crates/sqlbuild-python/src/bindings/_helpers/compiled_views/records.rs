//! Compiled records and views: projecting native views as records, and recording later stages.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{
    PyDict, PyDictMethods, PyFrozenSet, PyFrozenSetMethods, PyList, PyTuple, PyType,
};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;

use crate::bindings::_helpers::compiled_project::project::{
    AnalysisPy, NativeCompiledProject, ProofPy, model_facts,
};
use crate::bindings::_helpers::compiled_project::schema_facts::entry_schema_fact;
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::inputs_view::NativeInputsView;
use crate::bindings::_helpers::compiled_views::model_input_view::NativeModelInput;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::record_spec::record_spec;
use sqlbuild_python_base::records::types::RecordId;

const DATACLASSES_MODULE: &str = "dataclasses";
const DECIMAL_KIND: &str = "decimal";
const COLLECTION_KINDS: [&str; 2] = ["list", "set"];
const SCALAR_KINDS: [&str; 5] = ["string", "integer", "boolean", "float", "null"];

/// `dataclasses.replace(value, **changes)`.
pub(crate) fn replace<'py>(
    value: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for (name, change) in changes {
        keywords.set_item(*name, change)?;
    }
    imported(py, DATACLASSES_MODULE, "replace")?.call((value,), Some(&keywords))
}

/// `value` when it already is a `class` record, else `class(**{field: getattr(value, field)})`.
fn record<'py>(value: &Bound<'py, PyAny>, class: RecordId) -> PyResult<Bound<'py, PyAny>> {
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

/// The model as a `CompiledModel`, projected from its owner when it is a native view.
#[pyfunction]
pub(crate) fn compiled_model_record<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    record(model, RecordId::CompiledModel)
}

/// The audit as a `CompiledAudit`, projected from its owner when it is a native view.
#[pyfunction]
pub(crate) fn compiled_audit_record<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    record(audit, RecordId::CompiledAudit)
}

/// The function as a `CompiledFunction`, projected from its owner when it is a native view.
#[pyfunction]
pub(crate) fn compiled_function_record<'py>(
    function: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    record(function, RecordId::CompiledFunction)
}

/// The source as a `CompiledSource`, projected from its owner when it is a native view.
#[pyfunction]
pub(crate) fn compiled_source_record<'py>(
    source: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    record(source, RecordId::CompiledSource)
}

/// The seed as a `CompiledSeed`, projected from its owner when it is a native view.
#[pyfunction]
pub(crate) fn compiled_seed_record<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    record(seed, RecordId::CompiledSeed)
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

/// The model with these final binding diagnostics.
#[pyfunction]
#[pyo3(signature = (*, model, diagnostics))]
pub(crate) fn with_binding_diagnostics<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    if model.is_instance(&record_class(py, RecordId::CompiledModel)?)? {
        return replace(model, &[("binding_diagnostics", diagnostics.clone())]);
    }
    model.call_method1("with_binding_diagnostics", (diagnostics,))
}

/// The model with its recovered bindings and its poisoned outputs' types forgotten.
#[pyfunction]
#[pyo3(signature = (*, model, diagnostics, poisoned))]
pub(crate) fn with_type_recovery<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
    poisoned: &Bound<'py, PyFrozenSet>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let name: String = native_field(model, "name")?.extract()?;
    let mut unchecked: Vec<String> = Vec::new();
    for pair in poisoned.iter() {
        let (model_name, column): (String, String) = pair.extract()?;
        if model_name == name {
            unchecked.push(column);
        }
    }
    if !model.is_instance(&record_class(py, RecordId::CompiledModel)?)? {
        return model.call_method1("with_type_recovery", (diagnostics, unchecked));
    }
    let inferred: Bound<'py, PyAny> = native_field(model, "inferred_columns")?;
    let columns: Bound<'py, PyAny> = if inferred.is_none() {
        inferred
    } else {
        let mut columns: Vec<Bound<'py, PyAny>> = Vec::new();
        for column in inferred.try_iter()? {
            let column: Bound<'py, PyAny> = column?;
            let key = (name.as_str(), native_field(&column, "name")?);
            columns.push(if poisoned.contains(key)? {
                replace(&column, &[("type", py.None().into_bound(py))])?
            } else {
                column
            });
        }
        PyTuple::new(py, columns)?.into_any()
    };
    replace(
        model,
        &[
            ("binding_diagnostics", diagnostics.clone()),
            ("inferred_columns", columns),
            (
                "unchecked_output_columns",
                PyFrozenSet::new(py, &unchecked)?.into_any(),
            ),
        ],
    )
}

/// The project with `changes`; a native view shares its owner's models.
#[pyfunction]
#[pyo3(signature = (*, project, **changes))]
pub(crate) fn project_with<'py>(
    project: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    with_changes(project, RecordId::CompiledProject, changes)
}

/// The compile inputs with `changes`; a native view shares its owner's model inputs.
#[pyfunction]
#[pyo3(signature = (*, inputs, **changes))]
pub(crate) fn inputs_with<'py>(
    inputs: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    with_changes(inputs, RecordId::CompileProjectInputs, changes)
}

/// `replace(value, **changes)` for a record, else the native view's `with_changes(**changes)`.
fn with_changes<'py>(
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

/// The native owner's inputs view when it holds every model input; else a record.
#[pyfunction]
#[pyo3(signature = (*, fields, native_project))]
pub(crate) fn compile_inputs_view<'py>(
    fields: &Bound<'py, PyDict>,
    native_project: Option<Bound<'py, NativeCompiledProject>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = fields.py();
    if let Some(project) = &native_project {
        let mut native: bool = true;
        for model in fields.as_any().get_item("model_inputs")?.try_iter()? {
            if !model?.is_instance_of::<NativeModelInput>() {
                native = false;
                break;
            }
        }
        if native {
            let view: NativeInputsView =
                NativeCompiledProject::inputs_view(project.clone(), fields.clone())?;
            return view.into_bound_py_any(py);
        }
    }
    let keywords: Bound<'py, PyDict> = fields.copy()?;
    keywords.set_item("native_project", native_project)?;
    new_record(py, RecordId::CompileProjectInputs, (), Some(&keywords))
}

/// A typed SQL value as the plain value native consumers read.
#[pyfunction]
pub(crate) fn plain_typed_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let kind: String = native_field(value, "kind")?.getattr("value")?.extract()?;
    let inner: Bound<'py, PyAny> = native_field(value, "value")?;
    if kind == DECIMAL_KIND {
        return Ok(inner.str()?.into_any());
    }
    if SCALAR_KINDS.contains(&kind.as_str()) {
        return Ok(inner);
    }
    if COLLECTION_KINDS.contains(&kind.as_str()) {
        let items: Vec<Bound<'py, PyAny>> = inner
            .try_iter()?
            .map(|item| plain_typed_value(&item?))
            .collect::<PyResult<_>>()?;
        return Ok(PyList::new(py, items)?.into_any());
    }
    let mapping: Bound<'py, PyDict> = PyDict::new(py);
    for pair in inner.try_iter()? {
        let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        mapping.set_item(key, plain_typed_value(&item)?)?;
    }
    Ok(mapping.into_any())
}

/// An enum declaration as the native project row.
fn enum_row<'py>(declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
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
fn constant_row<'py>(declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
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

/// Enum and constant declarations as native rows, in the given order.
#[pyfunction]
#[pyo3(signature = (*, enums, constants))]
pub(crate) fn declaration_rows<'py>(
    enums: &Bound<'py, PyAny>,
    constants: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
    let py: Python<'py> = enums.py();
    let enum_rows: Vec<Bound<'py, PyTuple>> = enums
        .try_iter()?
        .map(|declaration| enum_row(&declaration?))
        .collect::<PyResult<_>>()?;
    let constant_rows: Vec<Bound<'py, PyTuple>> = constants
        .try_iter()?
        .map(|declaration| constant_row(&declaration?))
        .collect::<PyResult<_>>()?;
    Ok((PyList::new(py, enum_rows)?, PyList::new(py, constant_rows)?))
}

/// `(name, type)` of each dynamic family; `casefold` keys the family by its folded name.
fn family_types<'py>(
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

/// Transfer each assembled model's typed analysis to its retained facts; return how many.
#[pyfunction]
#[pyo3(signature = (*, project, models))]
pub(crate) fn record_model_analyses<'py>(
    project: &Bound<'py, NativeCompiledProject>,
    models: &Bound<'py, PyAny>,
) -> PyResult<usize> {
    let py: Python<'py> = project.py();
    let models: Vec<Bound<'py, PyAny>> = models.try_iter()?.collect::<PyResult<_>>()?;
    let views: Vec<PyRef<'py, NativeCompiledModel>> = models
        .iter()
        .filter(|model| model.is_instance_of::<NativeCompiledModel>())
        .map(|model| model.extract::<PyRef<'py, NativeCompiledModel>>())
        .collect::<PyResult<_>>()?;
    let mut transferred: usize = NativeCompiledProject::record_view_analyses(project, views)?;
    let nullable: Bound<'py, PyAny> = enum_member(py, RecordId::InferredNullability, "NULLABLE")?;
    for model in &models {
        if model.is_instance_of::<NativeCompiledModel>() {
            continue;
        }
        let inferred: Bound<'py, PyAny> = native_field(model, "inferred_rows")?;
        let rows: Vec<(String, Option<String>, Bound<'py, PyAny>)> = if inferred.is_none() {
            Vec::new()
        } else {
            inferred.extract()?
        };
        let proof: Bound<'py, PyAny> = native_field(model, "dynamic_column_contract")?;
        let proof = if proof.is_none() {
            None
        } else {
            let families: Bound<'py, PyAny> = native_field(&proof, "families")?;
            Some(ProofPy {
                output_proven: native_field(&proof, "output_proven")?.extract()?,
                bare_dynamic_pivot: native_field(&proof, "bare_dynamic_pivot")?.extract()?,
                families: family_types(&families, true)?,
                failure_reason: native_field(&proof, "failure_reason")?.extract()?,
                named_families: family_types(&families, false)?,
            })
        };
        let analysis = AnalysisPy {
            inferred: !inferred.is_none(),
            inferred_columns: rows
                .iter()
                .map(|(name, column_type, _)| (name.clone(), column_type.clone()))
                .collect(),
            nullable_columns: rows
                .iter()
                .map(|(_, _, nullability)| nullability.eq(&nullable))
                .collect::<PyResult<_>>()?,
            fast_lineage_has_star: native_field(model, "fast_lineage_has_star")?.is_truthy()?,
            unchecked: native_field(model, "unchecked_output_columns")?
                .try_iter()?
                .map(|column| column?.extract())
                .collect::<PyResult<_>>()?,
        };
        let name: String = native_field(model, "name")?.extract()?;
        transferred += usize::from(
            project
                .borrow_mut()
                .record_model_analysis(&name, analysis, proof),
        );
    }
    Ok(transferred)
}

/// A native project for a `CompiledProject` built without the compile-input stage.
fn hand_built_project<'py>(
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

/// The compile's retained project, or one recorded from a hand-built project.
#[pyfunction]
pub(crate) fn compiled_project_facts<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeCompiledProject>> {
    if let Ok(view) = project.downcast::<NativeProjectView>() {
        return Ok(view.get().owner().bind(project.py()).clone());
    }
    let retained: Bound<'py, PyAny> = native_field(project, "native_project")?;
    if !retained.is_none() {
        return Ok(retained.downcast_into()?);
    }
    hand_built_project(project)
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    for function in [
        wrap_pyfunction!(compiled_model_record, module)?,
        wrap_pyfunction!(compiled_audit_record, module)?,
        wrap_pyfunction!(compiled_function_record, module)?,
        wrap_pyfunction!(compiled_source_record, module)?,
        wrap_pyfunction!(compiled_seed_record, module)?,
        wrap_pyfunction!(compiled_project_record, module)?,
        wrap_pyfunction!(with_binding_diagnostics, module)?,
        wrap_pyfunction!(with_type_recovery, module)?,
        wrap_pyfunction!(project_with, module)?,
        wrap_pyfunction!(inputs_with, module)?,
        wrap_pyfunction!(compile_inputs_view, module)?,
        wrap_pyfunction!(plain_typed_value, module)?,
        wrap_pyfunction!(declaration_rows, module)?,
        wrap_pyfunction!(record_model_analyses, module)?,
        wrap_pyfunction!(compiled_project_facts, module)?,
    ] {
        module.add_function(function)?;
    }
    Ok(())
}
