//! Declaration resolution contexts built natively from a Python scope lookup's positions.

use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{FromPyObject, intern, pyclass, pymethods};

use crate::bindings::_helpers::scopes::declaration_resolution::ResolutionInputs;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_scopes::scope_index::main::classify_resource::classify_resource;
use sqlbuild_scopes::scope_index::models::{
    ConsumerResource, GrantEntry, VisibilityReason, VisibilityTable, VisibleEntry,
};

const ENUM_KIND: u8 = 0;
const CONSTANT_KIND: u8 = 1;
const MACRO_KIND: u8 = 2;

pub(crate) type TableRow = (
    Vec<usize>,
    HashMap<String, Vec<usize>>,
    HashMap<String, Vec<usize>>,
);
type VisibleRow<'py> = (Bound<'py, PyAny>, VisibleEntry, Option<Bound<'py, PyAny>>);

/// One declaration position: identity, first record, name, kinds, runtime value and key.
#[derive(FromPyObject)]
pub(crate) struct PositionRow(Py<PyAny>, Py<PyAny>, Py<PyAny>, u8, Py<PyAny>, u8, u32);

/// The Python classes and enum members the contexts are built from.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ContextClasses {
    visibility_record: Py<PyAny>,
    pub(crate) resolution_context: Py<PyAny>,
    reasons: Py<PyDict>,
}

/// Positions, visibility table and classes of one scope lookup.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct NativeDeclarationContexts {
    positions: Vec<PositionRow>,
    table: VisibilityTable,
    pub(crate) classes: ContextClasses,
    lookup: LookupMappings,
    pub(crate) resolution: ResolutionInputs,
}

/// The Python lookup mappings a matching resource's private positions and grants come from.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct LookupMappings {
    identity_keys: Py<PyDict>,
    private_positions: Py<PyAny>,
    grants_by_resource: Py<PyAny>,
}

/// Dictionaries one context is assembled into, in Python's insertion order.
struct ContextParts<'py> {
    enums: Bound<'py, PyDict>,
    constants: Bound<'py, PyDict>,
    macros: Bound<'py, PyDict>,
    macro_records: Bound<'py, PyDict>,
    inaccessible: [Bound<'py, PyDict>; 3],
    visibility: [Bound<'py, PyDict>; 3],
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
    fn resolve<'py>(
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
    pub(crate) fn classified_context<'py>(
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
    fn grants<'py>(
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

    fn assemble<'py>(
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

    fn add_visible_value<'py>(
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

    fn visibility_record<'py>(
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
    pub(crate) fn visibility_record_for<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDeclarationContexts>()?;
    Ok(())
}
