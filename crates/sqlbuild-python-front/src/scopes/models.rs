//! Public records of scopes.

use crate::scopes::_helpers::checks::scope_validation::replace;
use crate::scopes::_helpers::declarations::declaration_contexts::CONSTANT_KIND;
use crate::scopes::_helpers::declarations::declaration_contexts::ContextClasses;
use crate::scopes::_helpers::declarations::declaration_contexts::ContextParts;
use crate::scopes::_helpers::declarations::declaration_contexts::ENUM_KIND;
use crate::scopes::_helpers::declarations::declaration_contexts::LookupMappings;
use crate::scopes::_helpers::declarations::declaration_contexts::MACRO_KIND;
use crate::scopes::_helpers::declarations::declaration_contexts::PositionRow;
use crate::scopes::_helpers::declarations::declaration_contexts::TableRow;
use crate::scopes::_helpers::declarations::declaration_contexts::VisibleRow;
use crate::scopes::_helpers::declarations::declaration_resolution::ResolutionInputs;
use crate::scopes::_helpers::index::scope_index::PATHLIB_MODULE;
use crate::scopes::_helpers::index::scope_index::items;
use crate::scopes::_helpers::index::scope_index::scenario_row;
use crate::scopes::_helpers::index::scope_index::test_file_row;
use crate::scopes::_helpers::index::scope_index::unindexable;
use crate::scopes::_helpers::relationships::relationship_scan::RelationshipScanSettings;
use crate::scopes::_helpers::relationships::relationship_scan::ScenarioRow;
use crate::scopes::_helpers::relationships::relationship_scan::TestFileRow;
use crate::scopes::_helpers::relationships::relationship_scan::scanned_relationships;
use pyo3::IntoPyObjectExt;
use pyo3::intern;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::relationship_names::models::RelationshipScan;
use sqlbuild_scopes::scope_index::main::classify_resource::classify_resource;
use sqlbuild_scopes::scope_index::main::relationship_grants::relationship_grants;
use sqlbuild_scopes::scope_index::models::ConsumerResource;
use sqlbuild_scopes::scope_index::models::GrantEntry;
use sqlbuild_scopes::scope_index::models::GrantRecord;
use sqlbuild_scopes::scope_index::models::GrantThrough;
use sqlbuild_scopes::scope_index::models::RelationshipFact;
use sqlbuild_scopes::scope_index::models::ResourceKind;
use sqlbuild_scopes::scope_index::models::ScopeDeferral;
use sqlbuild_scopes::scope_index::models::ScopeIndex;
use sqlbuild_scopes::scope_index::models::VisibilityReason;
use sqlbuild_scopes::scope_index::models::VisibilityTable;
use sqlbuild_scopes::scope_index::models::VisibleEntry;
use std::collections::HashMap;

/// Positions, visibility table and classes of one scope lookup.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeDeclarationContexts {
    pub(crate) positions: Vec<PositionRow>,
    pub(crate) table: VisibilityTable,
    pub(crate) classes: ContextClasses,
    pub(crate) lookup: LookupMappings,
    pub(crate) resolution: ResolutionInputs,
}

#[pymethods]
impl NativeDeclarationContexts {
    #[new]
    pub(crate) fn new(
        positions: Vec<PositionRow>,
        table: TableRow,
        classes: ContextClasses,
        lookups: (LookupMappings, ResolutionInputs),
    ) -> Self {
        let (lookup, resolution) = lookups;
        let (global, local, inherited) = table;
        let identity_keys: Vec<u32> = positions.iter().map(|row| row.6).collect();
        Self {
            table: VisibilityTable {
                identity_keys,
                global,
                local,
                inherited,
            },
            positions,
            classes,
            lookup,
            resolution,
        }
    }

    /// `resolve_declaration_context`: the context and whether it was projected, not cached.
    #[pyo3(signature = (resolver, file_path, resource=None))]
    pub(crate) fn resolve<'py>(
        &self,
        resolver: Bound<'py, PyAny>,
        file_path: Bound<'py, PyAny>,
        resource: Option<Bound<'py, PyAny>>,
    ) -> PyResult<(Bound<'py, PyAny>, bool)> {
        compiler_guard(|| {
            let path: String = file_path.str()?.to_string();
            self.resolved_context(&resolver, &path, resource.as_ref())
        })
    }
}

impl NativeDeclarationContexts {
    pub fn classified_context<'py>(
        &self,
        py: Python<'py>,
        matches: Vec<Bound<'py, PyAny>>,
        consumer: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut visible: Vec<VisibleRow<'py>> = Vec::new();
        let mut inaccessible: Vec<usize> = Vec::new();
        for record in matches {
            let identity: Bound<'py, PyAny> = native_field(&record, "identity")?;
            let path: String = native_field(&record, "path")?.extract()?;
            let private: Vec<usize> = self
                .lookup
                .private_positions
                .bind(py)
                .call_method1(intern!(py, "get"), (&identity, PyTuple::empty(py)))?
                .extract()?;
            if private
                .iter()
                .any(|position| *position >= self.positions.len())
            {
                return Err(compiler_error(
                    "private declaration outside the scope lookup",
                ));
            }
            let (grants, throughs) = self.grants(py, &identity)?;
            let classified = classify_resource(
                &self.table,
                &ConsumerResource {
                    private,
                    path,
                    grants,
                },
            )
            .map_err(|deferral| compiler_error(deferral.reason))?;
            for entry in classified.visible {
                let through: Option<Bound<'py, PyAny>> =
                    entry.through.map(|slot| throughs[slot].clone());
                visible.push((identity.clone(), entry, through));
            }
            inaccessible.extend(classified.inaccessible);
        }
        self.assemble(py, &visible, &inaccessible, consumer)
    }

    /// The resource's grants of indexed declarations, with each grant's `through` value.
    pub(crate) fn grants<'py>(
        &self,
        py: Python<'py>,
        identity: &Bound<'py, PyAny>,
    ) -> PyResult<(Vec<GrantEntry>, Vec<Bound<'py, PyAny>>)> {
        let records: Bound<'py, PyAny> = self
            .lookup
            .grants_by_resource
            .bind(py)
            .call_method1(intern!(py, "get"), (identity, PyTuple::empty(py)))?;
        let identity_keys: &Bound<'py, PyDict> = self.lookup.identity_keys.bind(py);
        let mut grants: Vec<GrantEntry> = Vec::new();
        let mut throughs: Vec<Bound<'py, PyAny>> = Vec::new();
        for grant in records.try_iter()? {
            let grant: Bound<'py, PyAny> = grant?;
            let Some(identity_key) =
                identity_keys.get_item(native_field(&grant, "declaration")?)?
            else {
                continue;
            };
            let kind: String = native_field(&grant, "kind")?
                .getattr(intern!(py, "value"))?
                .extract()?;
            grants.push(GrantEntry {
                identity_key: identity_key.extract()?,
                reason: if kind == VisibilityReason::TestedMacro.as_str() {
                    VisibilityReason::TestedMacro
                } else {
                    VisibilityReason::ExpectedModel
                },
                through: throughs.len(),
            });
            throughs.push(native_field(&grant, "through")?);
        }
        Ok((grants, throughs))
    }

    pub(crate) fn assemble<'py>(
        &self,
        py: Python<'py>,
        visible: &[VisibleRow<'py>],
        inaccessible: &[usize],
        consumer: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let parts: ContextParts<'py> = ContextParts {
            enums: PyDict::new(py),
            constants: PyDict::new(py),
            macros: PyDict::new(py),
            macro_records: PyDict::new(py),
            inaccessible: [PyDict::new(py), PyDict::new(py), PyDict::new(py)],
            visibility: [PyDict::new(py), PyDict::new(py), PyDict::new(py)],
        };
        let mut records_by_identity: HashMap<u32, Vec<Bound<'py, PyAny>>> = HashMap::new();
        for (resource, entry, through) in visible {
            let record: Bound<'py, PyAny> = self.visibility_record(py, resource, entry, through)?;
            records_by_identity
                .entry(self.positions[entry.position].6)
                .or_default()
                .push(record);
        }
        for (_, entry, _) in visible {
            self.add_visible_value(py, &parts, entry.position)?;
        }
        for position in inaccessible {
            let row: &PositionRow = &self.positions[*position];
            if let Some(target) = parts.inaccessible.get(usize::from(row.3)) {
                target.set_item(row.2.bind(py), row.1.bind(py))?;
            }
        }
        for (_, entry, _) in visible {
            let row: &PositionRow = &self.positions[entry.position];
            if let Some(target) = parts.visibility.get(usize::from(row.3)) {
                let records: &[Bound<'py, PyAny>] =
                    records_by_identity.get(&row.6).map_or(&[], Vec::as_slice);
                target.set_item(row.2.bind(py), PyTuple::new(py, records)?)?;
            }
        }
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("enums", &parts.enums)?;
        fields.set_item("constants", &parts.constants)?;
        fields.set_item("inaccessible_enums", &parts.inaccessible[0])?;
        fields.set_item("inaccessible_constants", &parts.inaccessible[1])?;
        fields.set_item("enum_visibility", &parts.visibility[0])?;
        fields.set_item("constant_visibility", &parts.visibility[1])?;
        fields.set_item("macros", &parts.macros)?;
        fields.set_item("macro_records", &parts.macro_records)?;
        fields.set_item("macro_visibility", &parts.visibility[2])?;
        fields.set_item("inaccessible_macros", &parts.inaccessible[2])?;
        fields.set_item("consumer", consumer)?;
        self.classes
            .resolution_context
            .bind(py)
            .call((), Some(&fields))
    }

    pub(crate) fn add_visible_value<'py>(
        &self,
        py: Python<'py>,
        parts: &ContextParts<'py>,
        position: usize,
    ) -> PyResult<()> {
        let row: &PositionRow = &self.positions[position];
        let name: &Bound<'py, PyAny> = row.2.bind(py);
        let value: &Bound<'py, PyAny> = row.4.bind(py);
        match row.5 {
            ENUM_KIND => parts.enums.set_item(name, value),
            CONSTANT_KIND => parts.constants.set_item(name, value),
            MACRO_KIND => {
                parts.macros.set_item(name, value)?;
                parts.macro_records.set_item(name, row.1.bind(py))
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn visibility_record<'py>(
        &self,
        py: Python<'py>,
        resource: &Bound<'py, PyAny>,
        entry: &VisibleEntry,
        through: &Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let none: Bound<'py, PyAny> = py.None().into_bound(py);
        let reason: Bound<'py, PyAny> = self
            .classes
            .reasons
            .bind(py)
            .get_item(entry.reason.as_str())?
            .unwrap_or_else(|| none.clone());
        self.visibility_record_for(
            resource,
            self.positions[entry.position].0.bind(py),
            &reason,
            through.as_ref().unwrap_or(&none),
        )
    }

    /// A `VisibilityRecord` of `resource`, `declaration`, `reason` and `through`.
    pub fn visibility_record_for<'py>(
        &self,
        resource: &Bound<'py, PyAny>,
        declaration: &Bound<'py, PyAny>,
        reason: &Bound<'py, PyAny>,
        through: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = resource.py();
        let class: &Bound<'py, PyAny> = self.classes.visibility_record.bind(py);
        let record: Bound<'py, PyAny> = class.call1((resource, declaration, reason, through))?;
        Ok(record)
    }
}

/// A built scope index and the Python records materialised from it.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeScopeIndex {
    pub(crate) native: ScopeIndex,
    /// Each declaration record in input order.
    pub(crate) declarations: Vec<Py<PyAny>>,
    pub(crate) index: Py<PyAny>,
    pub(crate) grants: Py<PyTuple>,
}

#[pymethods]
impl NativeScopeIndex {
    /// The static index, without grants.
    #[getter]
    pub fn index(&self, py: Python<'_>) -> Py<PyAny> {
        self.index.clone_ref(py)
    }

    /// Whether any macro, enum or constant is scoped, which makes relationship grants matter.
    #[getter]
    pub fn has_scoped_relationship_declarations(&self) -> bool {
        self.native.has_scoped_relationship_declarations
    }

    /// Grant every test's and scenario's relationships; return each extraction fault.
    #[pyo3(signature = (*, discovered_inputs, syntax, scan_store))]
    pub(crate) fn grant_relationships(
        &mut self,
        py: Python<'_>,
        discovered_inputs: &Bound<'_, PyAny>,
        syntax: &Bound<'_, PyAny>,
        scan_store: (Bound<'_, PyAny>, String),
    ) -> PyResult<Py<PyTuple>> {
        let files: Vec<TestFileRow> = items(discovered_inputs, "test_files")?
            .iter()
            .map(test_file_row)
            .collect::<PyResult<_>>()?;
        let scenarios: Vec<ScenarioRow> = items(discovered_inputs, "scenario_files")?
            .iter()
            .map(scenario_row)
            .collect::<PyResult<_>>()?;
        let (store, algorithm) = scan_store;
        let settings: Bound<'_, PyDict> = PyDict::new(py);
        settings.set_item("syntax", native_field(syntax, "native_mapping")?)?;
        settings.set_item("store", store)?;
        settings.set_item("scan_key", (algorithm, native_field(syntax, "cache_key")?))?;
        let settings: RelationshipScanSettings = settings.extract()?;
        let scan: RelationshipScan = scanned_relationships(py, &files, &scenarios, settings)?;
        let index: &ScopeIndex = &self.native;
        let facts: &[RelationshipFact] = &scan.facts;
        let grants: Result<Vec<GrantRecord>, ScopeDeferral> = py
            .compiler_detach(|| Ok(relationship_grants(index, facts)))
            .map_err(compiler_error)?;
        let Ok(grants) = grants else {
            return Err(unindexable(py)?);
        };
        let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
        let mut records: Vec<Bound<'_, PyAny>> = Vec::with_capacity(grants.len());
        for grant in &grants {
            let through: Bound<'_, PyAny> = match &grant.through {
                GrantThrough::Model(name) => classes.model(RecordId::ResourceIdentity)?.call1((
                    classes.member(RecordId::ResourceKind, ResourceKind::Model.as_str())?,
                    name.as_str(),
                ))?,
                GrantThrough::Declaration(position) => {
                    self.declarations[*position].bind(py).getattr("identity")?
                }
            };
            records.push(
                classes.model(RecordId::GrantRecord)?.call1((
                    classes.model(RecordId::ResourceIdentity)?.call1((
                        classes.member(RecordId::ResourceKind, grant.resource.kind.as_str())?,
                        grant.resource.name.as_str(),
                    ))?,
                    self.declarations[grant.declaration]
                        .bind(py)
                        .getattr("identity")?,
                    through,
                    classes.member(RecordId::GrantKind, grant.kind.as_str())?,
                ))?,
            );
        }
        self.grants = PyTuple::new(py, records)?.unbind();
        let path_class: Bound<'_, PyAny> = py.import(PATHLIB_MODULE)?.getattr("Path")?;
        let faults: Vec<Bound<'_, PyAny>> = scan
            .faults
            .into_iter()
            .map(|(path, message)| {
                Ok(PyTuple::new(
                    py,
                    [path_class.call1((path,))?, message.into_bound_py_any(py)?],
                )?
                .into_any())
            })
            .collect::<PyResult<_>>()?;
        Ok(PyTuple::new(py, faults)?.unbind())
    }

    /// The index with its grants attached and relationships complete.
    pub(crate) fn index_with_relationships(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let index: &Bound<'_, PyAny> = self.index.bind(py);
        let completeness: Bound<'_, PyAny> = replace(
            &native_field(index, "completeness")?,
            &[("relationships", true.into_bound_py_any(py)?)],
        )?;
        Ok(replace(
            index,
            &[
                ("grants", self.grants.bind(py).clone().into_any()),
                ("completeness", completeness),
            ],
        )?
        .unbind())
    }
}

/// The native scope model and enum classes every built object comes from.
pub struct ScopeClasses<'py> {
    pub(crate) py: Python<'py>,
}

impl std::fmt::Debug for ScopeClasses<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopeClasses")
            .finish_non_exhaustive()
    }
}

impl<'py> ScopeClasses<'py> {
    /// The scope classes of `py`.
    pub fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self { py })
    }

    /// One scope model class.
    pub fn model(&self, name: RecordId) -> PyResult<Bound<'py, PyAny>> {
        record_class(self.py, name)
    }

    /// The member of scope enum `name` with `value`; an unknown value raises `ValueError`.
    pub fn member(&self, name: RecordId, value: &str) -> PyResult<Bound<'py, PyAny>> {
        new_record(self.py, name, (value,), None)
    }

    /// The interpreter the classes belong to.
    pub fn py(&self) -> Python<'py> {
        self.py
    }
}

/// Target, location, whether it is a directory, and the filters.
#[derive(Debug)]
pub struct ReportArguments<'py> {
    pub target: Option<Bound<'py, PyAny>>,
    pub at: Option<Bound<'py, PyAny>>,
    pub directory: bool,
    pub filters: Option<Bound<'py, PyAny>>,
}

/// Declaration, target, location and whether it is a directory.
#[derive(Debug)]
pub struct ExplainArguments<'py> {
    pub declaration: Bound<'py, PyAny>,
    pub target: Option<Bound<'py, PyAny>>,
    pub at: Option<Bound<'py, PyAny>>,
    pub directory: bool,
}

/// Folder, filters, target and whether the target is prospective.
#[derive(Debug)]
pub struct ListArguments<'py> {
    pub folder: String,
    pub filters: Option<Bound<'py, PyAny>>,
    pub target: Option<Bound<'py, PyAny>>,
    pub target_is_prospective: bool,
}
