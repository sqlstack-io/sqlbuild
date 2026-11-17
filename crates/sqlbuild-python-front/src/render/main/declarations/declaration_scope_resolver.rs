//! Pair the serializable static index with original process-local declaration values.

use crate::render::_helpers::declarations::declaration_values::TYPES_MODULE;
use crate::render::_helpers::declarations::declaration_values::native_declaration_contexts;
use crate::render::_helpers::declarations::declaration_values::resource_specific;
use crate::render::_helpers::declarations::declaration_values::runtime_declarations;
use crate::scopes::main::index::build_scope_lookup::build_scope_lookup;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Pair the serializable static index with original process-local declaration values.
pub fn declaration_scope_resolver<'py>(
    discovered: &Bound<'py, PyAny>,
    scope_index: &Bound<'py, PyAny>,
    (loaded_macros, lookup): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let declarations: Bound<'py, PyDict> = runtime_declarations(discovered, loaded_macros)?;
    let lookup: Bound<'py, PyAny> = if lookup.is_none() {
        build_scope_lookup(py, scope_index)?.into_bound(py)
    } else {
        lookup.clone()
    };
    let projection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::DeclarationRuntimeProjection,
        &[(
            "declarations",
            imported(py, TYPES_MODULE, "MappingProxyType")?.call1((&declarations,))?,
        )],
    )?;
    let native_contexts: Bound<'py, PyAny> =
        native_declaration_contexts(&lookup, declarations.as_any())?;
    new_record_with(
        py,
        RecordId::DeclarationScopeResolver,
        &[
            ("project_dir", native_field(discovered, "project_dir")?),
            ("lookup", lookup),
            ("projection", projection),
            (
                "resource_specific",
                resource_specific(scope_index)?.into_any(),
            ),
            ("native_contexts", native_contexts),
        ],
    )
}
