//! Audits attached by the native loop, held by the compiled project, and their views.

use crate::resource_views::models::NativeAuditInput;
use crate::resource_views::models::NativeCompiledAudit;
use crate::resource_views::types::AttachedAudit;
use crate::resource_views::types::AuditRecord;
use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{FromPyObject, pymethods};

use sqlbuild_project::types::ObjectKeyRow;

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::main::compiled_models::object_key::object_key;
use crate::compiled_views::main::model_inputs::reference_tuple::reference_tuple;
use crate::compiled_views::main::model_inputs::view_classes::view_classes;
use crate::compiled_views::models::InputViewClasses;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::scopes::main::reports::json_to_python::json_to_python;

const AUDIT_RESOURCE: &str = "audit";
const AUDIT_CLASSES: &str = "audits";
const NOT_ASSEMBLED: &str = "the audit was not assembled";

/// The Python enums an audit view projects its severity and attached target kind with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct AuditClasses<'py> {
    audit_severity: Bound<'py, PyAny>,
    attached_target_kind: Bound<'py, PyAny>,
}

/// `(references as (kind, name, package), attached (kind, name))`.
pub(crate) type AuditAssemblyRow = (
    Vec<(String, String, Option<String>)>,
    Option<(String, String)>,
);

pub(crate) fn read_audit<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&AuditRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    let audit: &AuditRecord = owner
        .audits()
        .get(index)
        .ok_or_else(|| PyIndexError::new_err(index))?;
    read(audit, py)
}

fn audit_classes<'py>(
    owner: &Py<NativeCompiledProject>,
    py: Python<'py>,
) -> PyResult<AuditClasses<'py>> {
    owner
        .bind(py)
        .borrow()
        .input_classes(py)?
        .get_item(AUDIT_CLASSES)?
        .extract()
}

/// The attributes an audit's input and compiled views share.
macro_rules! audit_getters {
    ($view:ty, { $($extra:tt)* }) => {
        #[pymethods]
        impl $view {
            $($extra)*

            #[getter]
            pub(crate) fn audit_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(audit.attached.host.audit_file.clone_ref(py))
                })
            }

            #[getter]
            pub(crate) fn audit_block(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(audit.attached.host.audit_block.clone_ref(py))
                })
            }

            #[getter]
            pub(crate) fn sql_body(&self, py: Python<'_>) -> PyResult<String> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.sql_body.clone())
                })
            }

            #[getter]
            pub(crate) fn evaluation_mode(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(native_field(&audit
                        .attached
                        .host.audit_block
                        .bind(py), "evaluation_mode")?
                        .unbind())
                })
            }

            #[getter]
            pub(crate) fn measurement_contract(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(native_field(&audit
                        .attached
                        .host.audit_block
                        .bind(py), "measurement_contract")?
                        .unbind())
                })
            }

            #[getter]
            pub(crate) fn thresholds(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(audit.attached.host.thresholds.clone_ref(py))
                })
            }

            #[getter]
            pub(crate) fn minimum_samples(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(audit.attached.host.minimum_samples.clone_ref(py))
                })
            }

            #[getter]
            pub(crate) fn measure_sql(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit
                        .attached
                        .measurement
                        .then(|| audit.attached.sql_body.clone()))
                })
            }

            #[getter]
            pub(crate) fn evidence_sql(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.evidence_sql.clone())
                })
            }

            #[getter]
            pub(crate) fn evidence_limit(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    Ok(audit.attached.host.evidence_limit.clone_ref(py))
                })
            }

            #[getter]
            pub(crate) fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
                let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                read_audit(&self.owner, self.index, py, |audit, _| {
                    reference_tuple(&classes, &audit.attached.references, &audit.host.references)
                })
            }

            #[getter]
            pub(crate) fn attached_target_kind<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let classes: AuditClasses<'py> = audit_classes(&self.owner, py)?;
                read_audit(&self.owner, self.index, py, |audit, py| {
                    match &audit.attached.target {
                        Some(target) => classes.attached_target_kind.call1((&target.kind,)),
                        None => Ok(py.None().into_bound(py)),
                    }
                })
            }

            #[getter]
            pub fn attached_target_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.target.as_ref().map(|target| target.name.clone()))
                })
            }

            #[getter]
            pub(crate) fn attached_column_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit
                        .attached
                        .target
                        .as_ref()
                        .and_then(|target| target.column.clone()))
                })
            }

            #[getter]
            pub(crate) fn severity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let classes: AuditClasses<'py> = audit_classes(&self.owner, py)?;
                read_audit(&self.owner, self.index, py, |audit, _| {
                    classes.audit_severity.call1((audit.attached.severity,))
                })
            }

            #[getter]
            pub(crate) fn run_scope(&self, py: Python<'_>) -> PyResult<String> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.run_scope.clone())
                })
            }

            #[getter]
            pub(crate) fn always_run(&self, py: Python<'_>) -> PyResult<bool> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.always_run)
                })
            }

            #[getter]
            pub(crate) fn description(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_audit(&self.owner, self.index, py, |audit, _| {
                    Ok(audit.attached.description.clone())
                })
            }

            #[getter]
            pub(crate) fn meta(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                read_audit(&self.owner, self.index, py, |audit, py| {
                    match &audit.attached.meta {
                        Some(meta) => json_to_python(py, meta.clone(), false),
                        None => Ok(py.None()),
                    }
                })
            }
        }
    };
}

audit_getters!(NativeAuditInput, {
    #[getter]
    pub(crate) fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        read_audit(&self.owner, self.index, py, |audit, py| {
            Ok(audit.attached.host.usages.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_audit(&self.owner, self.index, py, |audit, _| {
            Ok(audit.attached.name.clone())
        })
    }

    /// `(references as (kind, name, package), attached (kind, name))` for project assembly.
    #[getter]
    pub(crate) fn assembly_row(&self, py: Python<'_>) -> PyResult<AuditAssemblyRow> {
        read_audit(&self.owner, self.index, py, |audit, _| {
            let attached: &AttachedAudit = &audit.attached;
            Ok((
                attached
                    .references
                    .iter()
                    .map(
                        |ReferenceRow {
                             kind,
                             name,
                             package,
                             ..
                         }| (kind.clone(), name.clone(), package.clone()),
                    )
                    .collect(),
                attached
                    .target
                    .as_ref()
                    .map(|target| (target.kind.clone(), target.name.clone())),
            ))
        })
    }
});

audit_getters!(NativeCompiledAudit, {
    /// The audit's planner-facing `CompiledObjectKey`.
    #[getter]
    pub(crate) fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_audit(&self.owner, self.index, py, |audit, py| {
            audit
                .host
                .key
                .get_or_try_init(py, || {
                    object_key(
                        &classes.compiled,
                        AUDIT_RESOURCE,
                        audit.attached.compiled_name(),
                    )
                })
                .map(|key| key.clone_ref(py))
        })
    }

    /// The planner-facing keys of the resources the audit is selected with.
    #[getter]
    pub(crate) fn scope_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_audit(&self.owner, self.index, py, |audit, py| {
            let deps: &[ObjectKeyRow] = audit
                .scope_deps
                .as_deref()
                .ok_or_else(|| PyRuntimeError::new_err(NOT_ASSEMBLED))?;
            let mut keys: Vec<Py<PyAny>> = Vec::with_capacity(deps.len());
            for (kind, name) in deps {
                keys.push(object_key(&classes.compiled, kind, name)?);
            }
            PyTuple::new(py, keys)
        })
    }

    #[getter]
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
        read_audit(&self.owner, self.index, py, |audit, _| {
            Ok(audit.attached.compiled_name().to_owned())
        })
    }

    #[getter]
    pub(crate) fn definition_name(&self, py: Python<'_>) -> PyResult<String> {
        read_audit(&self.owner, self.index, py, |audit, _| {
            Ok(audit.attached.definition_name.clone())
        })
    }
});
