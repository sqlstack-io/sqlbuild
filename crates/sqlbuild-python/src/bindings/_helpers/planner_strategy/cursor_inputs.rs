//! Cursor input relations and authored cursor input column checks of model plan entries.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyFrozenSet, PyString, PyTuple};

use sqlbuild_analysis::planner_strategy::main::cursor_input_column::cursor_input_column_failure;
use sqlbuild_analysis::planner_strategy::models::{CursorInputColumnCheck, CursorInputColumnOwner};

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::compile_attachment::cursor_roles::resolve_cursor_input_roles;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::cursor::scalars::parse_value;
use crate::bindings::_helpers::model_config::typed_config::{
    config_cursor_bound, config_text_object, config_value, model_values,
};
use crate::bindings::_helpers::planner_identity::metadata::or_empty;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_strategy::constants::{
    ENFORCED_CONTRACT, INCREMENTAL_MATERIALIZATION, MODEL_REFERENCE, SEED_REFERENCE,
    SOURCE_REFERENCE, TIMESTAMP_CURSOR,
};
use crate::bindings::_helpers::planner_strategy::lineage::resolve_lineage_reference;
use crate::bindings::_helpers::planner_strategy::models::CursorInputLookups;
use crate::bindings::_helpers::planner_strategy::values::{input_error, text};
use crate::bindings::_helpers::records::construct::{enum_from_value, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::references::source_relations::render_source_relation;

/// Whether a model is incremental with a cursor column.
fn incremental_cursor_flag(
    model: &Bound<'_, PyAny>,
    cursor_column: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let materialized: Option<Bound<'_, PyAny>> =
        config_text_object(model.py(), &model_values(model)?, "materialized")?;
    Ok(!cursor_column.is_none()
        && match materialized {
            Some(materialized) => materialized.eq(INCREMENTAL_MATERIALIZATION)?,
            None => false,
        })
}

/// The names of a sequence of objects with a `name`.
fn names(items: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    items
        .try_iter()?
        .map(|item| text(&native_field(&item?, "name")?))
        .collect()
}

/// Raise the planner error of a cursor input column its input does not expose.
fn check_column(check: CursorInputColumnCheck, py: Python<'_>) -> PyResult<()> {
    match cursor_input_column_failure(&check) {
        Some((message, code)) => Err(input_error(py, ErrorId::PlannerInput, &message, Some(code))),
        None => Ok(()),
    }
}

/// Cursor input lookups from keyword arguments, empty where the caller left one out.
fn lookups_of<'py>(request: &Request<'_, 'py>) -> PyResult<CursorInputLookups<'py>> {
    let py: Python<'py> = request.py();
    let lookup = |name: &str| or_empty(request.optional(name)?, py);
    Ok(CursorInputLookups {
        models_by_name: lookup("models_by_name")?.into(),
        functions_by_name: lookup("functions_by_name")?,
        source_map: lookup("source_map")?,
        source_warehouse_columns: lookup("source_warehouse_columns")?,
        model_locations: lookup("model_locations")?,
        seed_locations: lookup("seed_locations")?,
        runtime_cursor_producer_names: match request.optional("runtime_cursor_producer_names")? {
            Some(names) => names,
            None => PyFrozenSet::empty(py)?.into_any(),
        },
    })
}

/// Validate authored cursor input columns against contracts, then source warehouse metadata.
#[pyfunction(name = "validate_source_cursor_input_columns")]
#[pyo3(signature = (**request))]
pub(crate) fn cursor_input_columns_of<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<()> {
    let request = Request::new("validate_source_cursor_input_columns", request)?;
    request.get("source_warehouse_columns")?;
    reject_unexposed_cursor_input_columns(
        &request.get("model")?,
        &request.get("cursor_column")?,
        &lookups_of(&request)?,
    )
}

/// Reject authored cursor input columns a contract or the source warehouse metadata does not expose.
pub(crate) fn reject_unexposed_cursor_input_columns<'py>(
    model: &Bound<'py, PyAny>,
    cursor_column: &Bound<'py, PyAny>,
    lookups: &CursorInputLookups<'py>,
) -> PyResult<()> {
    let py: Python<'py> = model.py();
    if !incremental_cursor_flag(model, cursor_column)? {
        return Ok(());
    }
    let model: &Bound<'py, PyAny> = model;
    let roles: Bound<'py, PyAny> = resolve_cursor_input_roles(model)?;
    let models_by_name = &lookups.models_by_name;
    let source_map: &Bound<'py, PyAny> = &lookups.source_map;
    let warehouse_columns: &Bound<'py, PyAny> = &lookups.source_warehouse_columns;
    let model_name: String = model_reads::model_name(model)?;
    for role in ["filter_inputs", "watermark_inputs"] {
        for item in mapping_items(&native_field(&roles, role)?)?.try_iter()? {
            let (input_name, column): (String, String) = item?.extract()?;
            let reference: Bound<'py, PyAny> = resolve_lineage_reference(
                model,
                &input_name,
                models_by_name.clone_ref(py),
                lookups.functions_by_name.clone(),
            )?;
            let kind: String = text(&native_field(&reference, "ref_kind")?)?;
            let name: String = text(&native_field(&reference, "ref_name")?)?;
            let check = |owner, known_columns| CursorInputColumnCheck {
                model_name: model_name.clone(),
                owner,
                input_name: name.clone(),
                column: column.clone(),
                known_columns,
            };
            if kind == MODEL_REFERENCE {
                let upstream = models_by_name.get(py, &name)?;
                if upstream.is_none()
                    || !config_value(py, &model_values(&upstream)?, "contract")?
                        .eq(ENFORCED_CONTRACT)?
                {
                    continue;
                }
                let entry: Bound<'py, PyAny> = native_field(&upstream, "schema_entry")?;
                let declared: Vec<String> = if entry.is_none() {
                    Vec::new()
                } else {
                    names(&native_field(&entry, "columns")?)?
                };
                check_column(check(CursorInputColumnOwner::ModelContract, declared), py)?;
                continue;
            }
            if kind != SOURCE_REFERENCE {
                continue;
            }
            let source: Bound<'py, PyAny> = mapping_get(source_map, &name)?;
            if !source.is_none() && native_field(&source, "contract")?.eq(ENFORCED_CONTRACT)? {
                let declared: Vec<String> = names(&native_field(&source, "columns")?)?;
                check_column(check(CursorInputColumnOwner::SourceContract, declared), py)?;
                continue;
            }
            let known: Bound<'py, PyAny> = mapping_get(warehouse_columns, &name)?;
            if known.is_none() {
                continue;
            }
            check_column(
                check(CursorInputColumnOwner::SourceWarehouse, names(&known)?),
                py,
            )?;
        }
    }
    Ok(())
}

/// The qualified relation one cursor input reference reads, when it resolves.
fn input_relation<'py>(
    (lookups, adapter): (&CursorInputLookups<'py>, &Bound<'py, PyAny>),
    (kind, name): (&str, &str),
) -> PyResult<Option<String>> {
    let py: Python<'py> = adapter.py();
    let location: Bound<'py, PyAny> = match kind {
        MODEL_REFERENCE => {
            let model: Bound<'py, PyAny> = mapping_get(&lookups.model_locations, name)?;
            if model.is_none() {
                mapping_get(&lookups.seed_locations, name)?
            } else {
                model
            }
        }
        SEED_REFERENCE => mapping_get(&lookups.seed_locations, name)?,
        SOURCE_REFERENCE => {
            let source: Bound<'py, PyAny> = mapping_get(&lookups.source_map, name)?;
            if source.is_none() {
                return Ok(None);
            }
            return render_source_relation(&source, Some(adapter)).map(Some);
        }
        _ => py.None().into_bound(py),
    };
    if location.is_none() {
        return Ok(None);
    }
    native_field(&location, "qualified_name")?.extract()
}

/// The resource kind a resolved cursor input reference reads.
fn input_kind<'py>(
    py: Python<'py>,
    (kind, model_backed): (&str, bool),
) -> PyResult<Bound<'py, PyAny>> {
    let value: &str = match kind {
        MODEL_REFERENCE if model_backed => "model",
        MODEL_REFERENCE | SEED_REFERENCE => "seed",
        SOURCE_REFERENCE => "source",
        _ => return Ok(py.None().into_bound(py)),
    };
    enum_from_value(py, RecordId::GraphResourceKind, value)
}

/// A producer model's terminal cursor bound parsed with the consumer's cursor type.
fn terminal_bound<'py>(
    producer: Option<&Bound<'py, PyAny>>,
    (key, cursor_type): (&str, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = cursor_type.py();
    let Some(producer) = producer else {
        return Ok(py.None().into_bound(py));
    };
    match config_cursor_bound(py, &model_values(producer)?, key)? {
        Some(raw) => parse_value(&raw, cursor_type),
        None => Ok(py.None().into_bound(py)),
    }
}

/// Cursor-bearing input relations of an incremental model, for runtime range discovery.
#[pyfunction(name = "build_cursor_input_relations")]
#[pyo3(signature = (**request))]
pub(crate) fn cursor_input_relations_of<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("build_cursor_input_relations", request)?;
    for name in [
        "adapter",
        "model_locations",
        "models_by_name",
        "seed_locations",
        "source_map",
        "runtime_cursor_producer_names",
    ] {
        request.get(name)?;
    }
    build_cursor_input_relations(
        (&request.get("model")?, &request.get("adapter")?),
        &request.get("cursor_column")?,
        &lookups_of(&request)?,
    )
}

/// Cursor-bearing input relations of an incremental model, for runtime range discovery.
pub(crate) fn build_cursor_input_relations<'py>(
    (model, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    cursor_column: &Bound<'py, PyAny>,
    lookups: &CursorInputLookups<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    if !incremental_cursor_flag(model, cursor_column)? {
        return Ok(PyTuple::empty(py));
    }
    let model: Bound<'py, PyAny> = model.clone();
    let models_by_name = &lookups.models_by_name;
    let functions_by_name: &Bound<'py, PyAny> = &lookups.functions_by_name;
    let model_locations: &Bound<'py, PyAny> = &lookups.model_locations;
    let producers: &Bound<'py, PyAny> = &lookups.runtime_cursor_producer_names;
    let cursor_type: Bound<'py, PyAny> =
        match config_text_object(py, &model_values(&model)?, "cursor_type")? {
            Some(cursor_type) if cursor_type.is_truthy()? => cursor_type,
            _ => PyString::new(py, TIMESTAMP_CURSOR).into_any(),
        };
    let mut relations: Vec<Bound<'py, PyAny>> = Vec::new();
    let watermarks: Bound<'py, PyAny> =
        native_field(&resolve_cursor_input_roles(&model)?, "watermark_inputs")?;
    for item in mapping_items(&watermarks)?.try_iter()? {
        let (input_name, column): (String, Bound<'py, PyAny>) = item?.extract()?;
        let reference: Bound<'py, PyAny> = resolve_lineage_reference(
            &model,
            &input_name,
            models_by_name.clone_ref(py),
            functions_by_name.clone(),
        )?;
        let kind: String = text(&native_field(&reference, "ref_kind")?)?;
        let name: Bound<'py, PyAny> = native_field(&reference, "ref_name")?;
        let name_text: String = text(&name)?;
        let Some(relation) = input_relation((lookups, adapter), (&kind, &name_text))? else {
            continue;
        };
        let producer: Option<Bound<'py, PyAny>> = if kind == MODEL_REFERENCE {
            Some(models_by_name.get(py, &name_text)?).filter(|model| !model.is_none())
        } else {
            None
        };
        let grain: Bound<'py, PyAny> = match &producer {
            Some(producer) => config_text_object(py, &model_values(producer)?, "cursor_grain")?
                .unwrap_or_else(|| py.None().into_bound(py)),
            None => py.None().into_bound(py),
        };
        let model_backed: bool = kind == MODEL_REFERENCE && model_locations.contains(&name)?;
        relations.push(new_record_with(
            py,
            RecordId::CursorInputRelation,
            &[
                ("relation", PyString::new(py, &relation).into_any()),
                ("cursor_column", column),
                ("cursor_grain", grain),
                (
                    "is_model_backed",
                    PyBool::new(py, model_backed).to_owned().into_any(),
                ),
                (
                    "is_runtime_produced",
                    PyBool::new(py, producers.contains(&name)?)
                        .to_owned()
                        .into_any(),
                ),
                ("input_name", name.clone()),
                ("input_kind", input_kind(py, (&kind, model_backed))?),
                (
                    "terminal_cursor_start",
                    terminal_bound(producer.as_ref(), ("cursor_start", &cursor_type))?,
                ),
                (
                    "terminal_cursor_end",
                    terminal_bound(producer.as_ref(), ("cursor_end", &cursor_type))?,
                ),
            ],
        )?);
    }
    PyTuple::new(py, relations)
}

/// Selected models and seeds, and selected sources with loaders, whose relations this invocation produces.
#[pyfunction]
#[pyo3(signature = (*, scope, source_map))]
pub(crate) fn runtime_cursor_producer_names<'py>(
    scope: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = scope.py();
    let data = scope_data(scope)?;
    let mut names: Vec<&str> = Vec::new();
    for (kind, name) in &data.selected {
        match kind.as_str() {
            "model" | "seed" => names.push(name),
            "source" => {
                let source: Bound<'py, PyAny> = mapping_get(source_map, name)?;
                if !source.is_none() && !native_field(&source, "loader")?.is_none() {
                    names.push(name);
                }
            }
            _ => {}
        }
    }
    PyFrozenSet::new(py, names)
}
