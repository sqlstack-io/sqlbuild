//! Audits attached by the native loop, held by the compiled project, and their views.

use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{FromPyObject, IntoPyObjectExt, pyclass, pymethods};

use sqlbuild_project::models as project_models;
use sqlbuild_project::types::ObjectKeyRow;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::compiled_model_view::object_key;
use crate::bindings::_helpers::compiled_views::model_input_view::{
    InputViewClasses, reference_tuple, view_classes,
};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::scopes::scope_report_objects::json_to_python;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

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

/// What the audit loop attached for one singular audit block or attached generic audit.
pub(crate) type AttachedAudit = project_models::AttachedAudit<PyProjectHost>;

/// One attached audit held by the compiled project.
pub(crate) type AuditRecord = project_models::AuditRecord<PyProjectHost>;

/// An attached audit's user values.
pub(crate) struct AuditHandles {
    /// The discovered (or built-in) audit file and block the SQL comes from.
    pub(crate) audit_file: Py<PyAny>,
    pub(crate) audit_block: Py<PyAny>,
    /// The authored measurement policy specs, carrying user values.
    pub(crate) thresholds: Py<PyAny>,
    pub(crate) minimum_samples: Py<PyAny>,
    pub(crate) evidence_limit: Py<PyAny>,
    /// Usage records, the identities the scope index is built from.
    pub(crate) usages: Py<PyTuple>,
}

/// An audit record's Python projections, each built once on first access.
pub(crate) struct AuditProjections {
    pub(crate) references: PyOnceLock<Py<PyTuple>>,
    pub(crate) key: PyOnceLock<Py<PyAny>>,
}

/// The audit record of an attached audit, before assembly resolves its scope dependencies.
pub(crate) fn audit_record(attached: AttachedAudit) -> AuditRecord {
    AuditRecord {
        attached,
        scope_deps: None,
        host: AuditProjections {
            references: PyOnceLock::new(),
            key: PyOnceLock::new(),
        },
    }
}

fn read_audit<'py, T>(
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
            pub(crate) fn attached_target_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
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

/// One attached audit, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeAuditInput {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeAuditInput {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
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

/// One assembled audit, read from the compiled project that owns it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeCompiledAudit {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeCompiledAudit {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }
}

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

/// A native audit view's reference rows, or `None` for a hand-built audit record.
pub(crate) fn audit_reference_rows(
    audit: &Bound<'_, PyAny>,
) -> PyResult<Option<Vec<ReferenceRow>>> {
    let py: Python<'_> = audit.py();
    let (owner, index): (&Py<NativeCompiledProject>, usize) =
        if let Ok(view) = audit.cast::<NativeCompiledAudit>() {
            (&view.get().owner, view.get().index)
        } else if let Ok(view) = audit.cast::<NativeAuditInput>() {
            (&view.get().owner, view.get().index)
        } else {
            return Ok(None);
        };
    read_audit(owner, index, py, |record, _| {
        Ok(Some(record.attached.references.clone()))
    })
}

impl NativeCompiledAudit {
    /// The `CompiledAudit` field values, in field order, without building the record.
    pub(crate) fn record_values<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.key(py)?.into_bound(py),
            self.scope_deps(py)?.into_any(),
            self.name(py)?.into_bound_py_any(py)?,
            self.definition_name(py)?.into_bound_py_any(py)?,
            self.audit_file(py)?.into_bound(py),
            self.audit_block(py)?.into_bound(py),
            self.sql_body(py)?.into_bound_py_any(py)?,
            self.evaluation_mode(py)?.into_bound(py),
            self.measurement_contract(py)?.into_bound(py),
            self.thresholds(py)?.into_bound(py),
            self.minimum_samples(py)?.into_bound(py),
            self.measure_sql(py)?.into_bound_py_any(py)?,
            self.evidence_sql(py)?.into_bound_py_any(py)?,
            self.evidence_limit(py)?.into_bound(py),
            self.references(py)?.into_bound(py).into_any(),
            self.attached_target_kind(py)?,
            self.attached_target_name(py)?.into_bound_py_any(py)?,
            self.attached_column_name(py)?.into_bound_py_any(py)?,
            self.severity(py)?,
            self.run_scope(py)?.into_bound_py_any(py)?,
            pyo3::types::PyBool::new(py, self.always_run(py)?)
                .to_owned()
                .into_any(),
            self.description(py)?.into_bound_py_any(py)?,
            self.meta(py)?.into_bound(py),
        ])
    }
}
