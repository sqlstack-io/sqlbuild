//! Inference-profile settings crossing into native analysis: rule ids, shapes and the catalog.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::analysis::binding_catalog::{
    BindingCatalog, CASE_SENSITIVE_BINDING_DIALECTS, CatalogKey, NATIVE_DIALECT_ALIASES,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;

const GENERIC_DIALECT: &str = "generic";
pub(crate) const ADAPTER_RULE_ID: &str = "python";
/// The nullability rules the native analysis knows, with their native rule ids.
const KNOWN_RULES: [(AdapterHost, &str); 3] = [
    (AdapterHost::FirstArgNullability, "first_arg"),
    (
        AdapterHost::ConditionalResultNullability,
        "conditional_result",
    ),
    (AdapterHost::SafeCastNullability, "safe_cast"),
];

/// The adapter's own nullability rules and `InferredNullability`, which native analysis calls.
pub(crate) type AdapterRules = Option<(Py<PyDict>, Py<PyAny>)>;

/// The adapter nullability rules as `(name, rule id)` rows, and the adapter's own rules.
pub(crate) fn nullability_rules(profile: &Bound<'_, PyAny>) -> PyResult<(Pairs, AdapterRules)> {
    let py: Python<'_> = profile.py();
    let known: Bound<'_, PyDict> = PyDict::new(py);
    for (rule, id) in KNOWN_RULES {
        known.set_item(host::entry(py, rule)?, id)?;
    }
    let adapter: Bound<'_, PyDict> = PyDict::new(py);
    let mut rows: Pairs = Vec::new();
    for item in native_field(profile, "function_nullability_rules")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, rule): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        let id: String = match known.get_item(&rule)? {
            Some(id) => id.extract()?,
            None => {
                adapter.set_item(&name, &rule)?;
                ADAPTER_RULE_ID.to_owned()
            }
        };
        rows.push((name.extract()?, id));
    }
    if adapter.is_empty() {
        return Ok((rows, None));
    }
    let nullability: Bound<'_, PyAny> = record_class(py, RecordId::InferredNullability)?;
    Ok((rows, Some((adapter.unbind(), nullability.unbind()))))
}

/// `{relation: {column: value}}` as ordered rows, each value as `str(value)`.
pub(crate) fn shape_rows(shapes: &Bound<'_, PyAny>) -> PyResult<Shapes> {
    let mut rows: Shapes = Vec::new();
    for item in mapping_items(shapes)?.try_iter()? {
        let (name, shape): (String, Bound<'_, PyAny>) = item?.extract()?;
        rows.push((name, text_pairs(&shape)?));
    }
    Ok(rows)
}

/// `{column: value}` as ordered pairs, each value as `str(value)`.
pub(crate) fn text_pairs(shape: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    let mut pairs: Pairs = Vec::new();
    for column in mapping_items(shape)?.try_iter()? {
        let (column, value): (String, Bound<'_, PyAny>) = column?.extract()?;
        pairs.push((column, value.str()?.extract()?));
    }
    Ok(pairs)
}

/// A `{key: value}` text mapping's items in order.
pub(crate) fn pairs(mapping: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    mapping_items(mapping)?
        .try_iter()?
        .map(|item| item?.extract())
        .collect()
}

/// `profile.sql_analysis_dialect or "generic"`.
pub(crate) fn profile_dialect(profile: &Bound<'_, PyAny>) -> PyResult<String> {
    dialect_or_generic(&native_field(profile, "sql_analysis_dialect")?)
}

/// `dialect or "generic"`.
pub(crate) fn dialect_or_generic(dialect: &Bound<'_, PyAny>) -> PyResult<String> {
    if dialect.is_truthy()? {
        return dialect.extract();
    }
    Ok(GENERIC_DIALECT.to_owned())
}

/// Python's `NATIVE_DIALECT_ALIASES.get(dialect, dialect)`.
pub(crate) fn aliased_dialect(_py: Python<'_>, dialect: String) -> PyResult<String> {
    Ok(NATIVE_DIALECT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == dialect)
        .map_or(dialect.clone(), |(_, native)| (*native).to_owned()))
}

/// Python's `inferred_binding_shape` test for keeping authored identifier quoting.
pub(crate) fn case_sensitive_shapes(
    profile: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if native_field(profile, "quoted_identifiers_ignore_case")?.is_truthy()? {
        return Ok(false);
    }
    if dialect.is_none() {
        return Ok(false);
    }
    let dialect: String = dialect.extract()?;
    Ok(CASE_SENSITIVE_BINDING_DIALECTS.contains(&dialect.as_str()))
}

/// `profile.binding_catalog or create_binding_catalog(...)` with no functions, types or relations.
pub(crate) fn profile_catalog<'py>(
    profile: &Bound<'py, PyAny>,
    dialect: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let catalog: Bound<'py, PyAny> = native_field(profile, "binding_catalog")?;
    if catalog.is_truthy()? {
        return Ok(catalog);
    }
    let py: Python<'py> = profile.py();
    let catalog: BindingCatalog = BindingCatalog::create(
        py,
        CatalogKey {
            dialect: dialect.to_owned(),
            quoted_ignore_case: native_field(profile, "quoted_identifiers_ignore_case")?
                .extract()?,
            known_functions: Vec::new(),
            known_types: Vec::new(),
        },
        PyDict::new(py).as_any(),
    )?;
    Ok(Bound::new(py, catalog)?.into_any())
}

/// `profile.function_return_types` items in order.
pub(crate) fn return_types(profile: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    pairs(&native_field(profile, "function_return_types")?)
}
