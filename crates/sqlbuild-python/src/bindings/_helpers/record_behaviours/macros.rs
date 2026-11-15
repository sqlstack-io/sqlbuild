//! Behaviour of the natively hosted macro records and the compiled project's lineage graph.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyList, PyTuple, PyTupleMethods};

use crate::bindings::_helpers::graph::project_graph::{ResourceRow, project_graph};
use crate::bindings::_helpers::record_behaviours::compiled::compile_input_error;
use crate::bindings::_helpers::render::declaration_values::{
    constant_declaration_sql, render_enum_member_value,
};
use sqlbuild_python_base::boundary::main::call_argument::call_argument;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The parameter name through which a macro receives its context.
const MACRO_CONTEXT_PARAMETER_NAME: &str = "ctx";
/// Parameter kinds that can receive the context positionally.
const POSITIONAL_KINDS: [&str; 2] = ["POSITIONAL_ONLY", "POSITIONAL_OR_KEYWORD"];

/// `LoadedMacro.injects_context`: whether the first positional parameter is the macro context.
pub(crate) fn macro_injects_context(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let inspect: Bound<'_, PyAny> = py.import("inspect")?.into_any();
    let parameters: Bound<'_, PyTuple> = PyTuple::new(
        py,
        native_field(&inspect, "signature")?
            .call1((native_field(instance, "function")?,))?
            .getattr("parameters")?
            .call_method0("values")?
            .try_iter()?
            .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()?,
    )?;
    let Ok(first) = parameters.get_item(0) else {
        return false.into_py_any(py);
    };
    let kind: Bound<'_, PyAny> = native_field(&first, "kind")?;
    let mut positional: bool = false;
    for name in POSITIONAL_KINDS {
        positional |= kind.eq(native_field(&inspect, "Parameter")?.getattr(name)?)?;
    }
    let named: bool = native_field(&first, "name")?.eq(MACRO_CONTEXT_PARAMETER_NAME)?;
    (positional && named).into_py_any(py)
}

/// `MacroContext.render_constant(name)`: one visible constant as an adapter-safe SQL value.
pub(crate) fn context_render_constant(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let name: Bound<'_, PyAny> = call_argument((args, kwargs), "name", 0)?;
    let value_renderer: Bound<'_, PyAny> = native_field(instance, "_value_renderer")?;
    if value_renderer.is_none() {
        return Err(compile_input_error(
            py,
            "Constant SQL rendering is available only during a project macro invocation",
        )?);
    }
    let _ = native_field(instance, "constants")?.get_item(&name)?;
    let declaration: Bound<'_, PyAny> =
        native_field(instance, "_constant_declarations")?.call_method1("get", (&name,))?;
    if declaration.is_none() {
        return Err(compile_input_error(
            py,
            "Constant SQL rendering requires a declaration resolved for this macro invocation",
        )?);
    }
    constant_declaration_sql(
        &declaration,
        &value_renderer,
        (&native_field(instance, "_collection_rendering")?, None),
    )?
    .into_py_any(py)
}

/// `MacroContext.render_enum_member(*, enum_name, member_name)`: one member as a SQL literal.
pub(crate) fn context_render_enum_member(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    if !args.is_empty() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "render_enum_member() takes keyword arguments enum_name and member_name only",
        ));
    }
    let enum_name: Bound<'_, PyAny> = call_argument((args, kwargs), "enum_name", 0)?;
    let member_name: Bound<'_, PyAny> = call_argument((args, kwargs), "member_name", 1)?;
    let value: Bound<'_, PyAny> = native_field(instance, "enums")?
        .get_item(enum_name)?
        .get_item(member_name)?;
    render_enum_member_value(&value)?.into_py_any(instance.py())
}

/// `(resource type, name)` of one compiled object key.
fn graph_key(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(key, "resource_type")?.str()?.extract()?,
        native_field(key, "name")?.extract()?,
    ))
}

/// `(resource type, name)` of each compiled object key.
fn graph_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    keys.try_iter()?.map(|key| graph_key(&key?)).collect()
}

/// The texts of a list or tuple of tags; anything else has none.
fn graph_tags(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
        return Ok(Vec::new());
    }
    value
        .try_iter()?
        .map(|item| item?.str()?.extract())
        .collect()
}

/// A resource's tags and model folder.
type ResourceTags = (Vec<String>, Option<String>);
/// Reads one resource's tags and model folder.
type DescribeResource = fn(&Bound<'_, PyAny>) -> PyResult<ResourceTags>;

/// The graph rows of each resource in `resources`, with tags and folder from `describe`.
fn resource_rows(
    resources: &Bound<'_, PyAny>,
    describe: DescribeResource,
) -> PyResult<Vec<ResourceRow>> {
    let mut rows: Vec<ResourceRow> = Vec::new();
    for resource in resources.try_iter()? {
        let resource: Bound<'_, PyAny> = resource?;
        let (tags, folder) = describe(&resource)?;
        rows.push(ResourceRow {
            key: graph_key(&native_field(&resource, "key")?)?,
            deps: graph_keys(&native_field(&resource, "deps")?)?,
            tags,
            folder,
        });
    }
    Ok(rows)
}

/// A model's configured tags and folder.
fn model_tags(model: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let tags: Bound<'_, PyAny> =
        native_field(&native_field(model, "config")?, "values")?.call_method1("get", ("tags",))?;
    let folder: String = native_field(&native_field(model, "relative_path")?, "parent")?
        .str()?
        .extract()?;
    Ok((graph_tags(&tags)?, Some(folder)))
}

/// A source has neither tags nor folder.
fn source_tags(_source: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    Ok((Vec::new(), None))
}

/// A seed's schema entry tags.
fn seed_tags(seed: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let tags: Vec<String> = native_field(&native_field(seed, "schema_entry")?, "tags")?
        .try_iter()?
        .map(|tag| tag?.extract())
        .collect::<PyResult<_>>()?;
    Ok((tags, None))
}

/// A function's tags.
fn function_tags(function: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let tags: Vec<String> = native_field(function, "tags")?
        .try_iter()?
        .map(|tag| tag?.extract())
        .collect::<PyResult<_>>()?;
    Ok((tags, None))
}

/// `CompiledProject.lineage_graph`: lineage edges, tags, folders and names indexed natively.
pub(crate) fn project_lineage_graph(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let mut rows: Vec<ResourceRow> = Vec::new();
    rows.extend(resource_rows(
        &native_field(instance, "models")?,
        model_tags,
    )?);
    rows.extend(resource_rows(
        &native_field(instance, "sources")?,
        source_tags,
    )?);
    rows.extend(resource_rows(&native_field(instance, "seeds")?, seed_tags)?);
    rows.extend(resource_rows(
        &native_field(instance, "functions")?,
        function_tags,
    )?);
    project_graph(rows)?.into_py_any(instance.py())
}
