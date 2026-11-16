//! Runtime SQL relation maps for Python node contexts and hard-coded name guards.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PySet, PySetMethods, PyString};
use sqlbuild_host::entries::models::{AdapterHost, UserCodeHost};
use sqlbuild_python_base::records::main::construct::exception_from::exception_from;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::references::main::render_source_relation::render_source_relation;

const SHARED_EXCEPTIONS_MODULE: &str = "sqlbuild.errors.contracts.exceptions";

/// The `SqlResourceRefKind` members a relation map distinguishes.
pub(crate) struct RefKinds<'py> {
    model: Bound<'py, PyAny>,
    seed: Bound<'py, PyAny>,
    source: Bound<'py, PyAny>,
}

impl<'py> RefKinds<'py> {
    pub(crate) fn load(py: Python<'py>) -> PyResult<Self> {
        let kinds: Bound<'py, PyAny> = host_entry(py, UserCodeHost::SqlResourceRefKind)?;
        Ok(Self {
            model: native_field(&kinds, "MODEL")?,
            seed: native_field(&kinds, "SEED")?,
            source: native_field(&kinds, "SOURCE")?,
        })
    }
}

/// A `SharedInputError` with `message`.
fn shared_error(py: Python<'_>, message: String) -> PyErr {
    exception_from(
        imported(py, SHARED_EXCEPTIONS_MODULE, "SharedInputError"),
        &message,
        (None, None),
    )
}

/// The model, seed and Python source refs of a plan, as the frozenset Python builds.
pub(crate) fn planned_relation_refs<'py>(
    plan_output: &Bound<'py, PyAny>,
    kinds: &RefKinds<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan_output.py();
    let reference: Bound<'py, PyAny> = host_entry(py, UserCodeHost::SqlResourceRef)?;
    let refs: Bound<'py, PySet> = PySet::empty(py)?;
    for (attribute, kind) in [
        ("model_locations", &kinds.model),
        ("seed_locations", &kinds.seed),
        ("python_source_entries", &kinds.source),
    ] {
        for name in native_field(plan_output, attribute)?.try_iter()? {
            refs.add(call_with(
                &reference,
                &[("kind", kind.clone()), ("name", name?)],
            )?)?;
        }
    }
    imported(py, "builtins", "frozenset")?.call1((refs,))
}

/// `_resolve_relation`: the planned relation, or the compiled location's qualified name.
pub(crate) fn resolved_relation<'py>(
    (adapter, project, plan_output): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    reference: &Bound<'py, PyAny>,
    kinds: &RefKinds<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    let planned: Bound<'py, PyAny> = planned_relation(adapter, plan_output, reference, kinds)?;
    if !planned.is_none() {
        return Ok(planned);
    }
    let kind: Bound<'py, PyAny> = native_field(reference, "kind")?;
    let name: Bound<'py, PyAny> = native_field(reference, "name")?;
    if kind.eq(&kinds.source)? {
        return Err(shared_error(
            py,
            format!(
                "Python node source '{}' is missing from the resolved plan source map",
                name.str()?
            ),
        ));
    }
    let resources: Bound<'py, PyAny> = project.getattr(if kind.eq(&kinds.model)? {
        "models"
    } else {
        "seeds"
    })?;
    let mut location: Option<Bound<'py, PyAny>> = None;
    for resource in resources.try_iter()? {
        let resource: Bound<'py, PyAny> = resource?;
        if native_field(&resource, "name")?.eq(&name)? {
            location = Some(native_field(&resource, "destination")?);
            break;
        }
    }
    let Some(location) = location else {
        return Err(shared_error(
            py,
            format!(
                "Python node references unknown {} '{}'",
                native_field(&kind, "value")?.str()?,
                name.str()?
            ),
        ));
    };
    qualified_name(adapter, &location)
}

/// `resolve_relation_location_qualified_name(adapter=adapter, location=location)`.
fn qualified_name<'py>(
    adapter: &Bound<'py, PyAny>,
    location: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(
            adapter.py(),
            AdapterHost::ResolveRelationLocationQualifiedName,
        )?,
        &[("adapter", adapter.clone()), ("location", location.clone())],
    )
}

/// `_planned_relation`: the relation the plan resolved for `reference`, or `None`.
pub(crate) fn planned_relation<'py>(
    adapter: &Bound<'py, PyAny>,
    plan_output: &Bound<'py, PyAny>,
    reference: &Bound<'py, PyAny>,
    kinds: &RefKinds<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    let kind: Bound<'py, PyAny> = native_field(reference, "kind")?;
    let name: Bound<'py, PyAny> = native_field(reference, "name")?;
    if kind.eq(&kinds.source)? {
        let source: Bound<'py, PyAny> =
            native_field(plan_output, "python_source_entries")?.call_method1("get", (name,))?;
        if source.is_none() {
            return Ok(source);
        }
        let rendered: String = render_source_relation(&source, Some(adapter))?;
        return Ok(PyString::new(py, &rendered).into_any());
    }
    let locations: Bound<'py, PyAny> = plan_output.getattr(if kind.eq(&kinds.model)? {
        "model_locations"
    } else {
        "seed_locations"
    })?;
    let location: Bound<'py, PyAny> = mapping_get(&locations, name)?;
    if location.is_none() {
        return Ok(location);
    }
    qualified_name(adapter, &location)
}
