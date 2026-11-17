//! Inference-profile settings crossing into native analysis: rule ids, shapes and the catalog.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_host::entries::models::AdapterHost;

use crate::analysis::constants::NATIVE_DIALECT_ALIASES;
use crate::analysis::models::{BindingCatalog, CatalogKey};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

pub(crate) const GENERIC_DIALECT: &str = "generic";

/// The nullability rules the native analysis knows, with their native rule ids.
pub(crate) const KNOWN_RULES: [(AdapterHost, &str); 3] = [
    (AdapterHost::FirstArgNullability, "first_arg"),
    (
        AdapterHost::ConditionalResultNullability,
        "conditional_result",
    ),
    (AdapterHost::SafeCastNullability, "safe_cast"),
];

/// The adapter's own nullability rules and `InferredNullability`, which native analysis calls.
pub(crate) type AdapterRules = Option<(Py<PyDict>, Py<PyAny>)>;

/// A `{key: value}` text mapping's items in order.
pub(crate) fn pairs(mapping: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    mapping_items(mapping)?
        .try_iter()?
        .map(|item| item?.extract())
        .collect()
}

/// Python's `NATIVE_DIALECT_ALIASES.get(dialect, dialect)`.
pub(crate) fn aliased_dialect(_py: Python<'_>, dialect: String) -> PyResult<String> {
    Ok(NATIVE_DIALECT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == dialect)
        .map_or(dialect.clone(), |(_, native)| (*native).to_owned()))
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
