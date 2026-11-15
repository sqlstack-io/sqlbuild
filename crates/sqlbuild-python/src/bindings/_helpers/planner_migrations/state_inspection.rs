//! Read-only warehouse evidence gathered once for all model migrations in one plan.

use crate::bindings::_helpers::fingerprints::storage_io::read_latest_fingerprints;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods,
};
use pyo3::types::{PyBool, PyDict, PyString, PyTuple, PyTupleMethods};
use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_state::events::main::newest_fingerprint::newest_fingerprint;
use sqlbuild_state::events::models::{EventRelation, FingerprintKey, FingerprintOrder};

use crate::bindings::_helpers::planner_migrations::constants::{
    GET_COLUMNS_FOR_INSPECTION, LIST_RELATIONS_FOR_INSPECTION,
};
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::state_rows::constants::{COLUMN_KIND, MODEL_KIND, OLD_NAME_KIND};
use crate::bindings::_helpers::state_rows::migration_io::{read_events, stored_old_name_columns};
use crate::bindings::_helpers::state_rows::migration_objects::layout;
use crate::bindings::_helpers::state_rows::migration_projections::old_name_views;
use crate::bindings::_helpers::state_rows::models::StateAccess;
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_MODEL;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

/// A lowercased schema and relation name.
type RelationKey = (String, String);

/// Relations, fingerprints and migration events for one planning database.
#[pyclass(module = "sqlbuild._native", name = "MigrationStateInspection")]
pub(crate) struct MigrationStateInspection {
    adapter: Py<PyAny>,
    connection: Py<PyAny>,
    database: Py<PyAny>,
    known_fingerprints: Py<PyAny>,
    read_schemas: HashSet<String>,
    /// Model fingerprints per lowercased schema, in first-read order.
    fingerprints: Vec<(String, Vec<Py<PyAny>>)>,
    events: Vec<Py<PyAny>>,
    event_schemas: HashSet<String>,
    relations: HashMap<RelationKey, Py<PyAny>>,
    columns: HashMap<RelationKey, Py<PyAny>>,
    column_event_schemas: HashSet<String>,
    column_events: Vec<Py<PyAny>>,
    old_name_schemas: HashSet<String>,
    old_name_moves: Vec<Py<PyAny>>,
    old_name_facts: Vec<Py<PyAny>>,
}

fn lowered(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("lower")?.extract()
}

fn schema_or_empty<'py>(value: Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if value.is_none() {
        PyString::new(value.py(), "").into_any()
    } else {
        value
    }
}

/// `((schema or "").lower(), name.lower())` of a relation or location.
fn relation_key(value: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    Ok((
        lowered(&schema_or_empty(native_field(value, "schema")?))?,
        lowered(&native_field(value, "name")?)?,
    ))
}

/// The requested schemas not inspected yet, sorted, each with its lowercased key.
fn pending<'py>(
    schemas: &Bound<'py, PyAny>,
    inspected: &HashSet<String>,
) -> PyResult<Vec<(Bound<'py, PyAny>, String)>> {
    let mut pending: Vec<(String, Bound<'py, PyAny>, String)> = Vec::new();
    for schema in schemas.try_iter()? {
        let schema: Bound<'py, PyAny> = schema?;
        let key: String = lowered(&schema)?;
        if !inspected.contains(&key) {
            pending.push((schema.extract()?, schema, key));
        }
    }
    pending.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(pending
        .into_iter()
        .map(|(_, schema, key)| (schema, key))
        .collect())
}

fn tuple_of<'py>(py: Python<'py>, items: &[Py<PyAny>]) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, items.iter().map(|item| item.bind(py)))
}

fn fingerprint_key(fingerprint: &Bound<'_, PyAny>) -> PyResult<FingerprintKey> {
    let target_name: Bound<'_, PyAny> = native_field(fingerprint, "target_name")?;
    let target: Option<EventRelation> = if target_name.is_none() {
        None
    } else {
        Some(EventRelation {
            database: native_field(fingerprint, "target_database")?.extract()?,
            schema: native_field(fingerprint, "target_schema")?.extract()?,
            name: target_name.extract()?,
        })
    };
    Ok(FingerprintKey {
        order: FingerprintOrder {
            timestamp: native_field(fingerprint, "ts")?
                .call_method0("isoformat")?
                .extract()?,
            run_id: native_field(fingerprint, "run_id")?.extract()?,
        },
        node_name: native_field(fingerprint, "node_name")?.extract()?,
        target,
    })
}

fn newest<'py>(
    py: Python<'py>,
    candidates: &[Py<PyAny>],
    (target, model_name): (Option<&EventRelation>, Option<&str>),
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let keys: Vec<FingerprintKey> = candidates
        .iter()
        .map(|candidate| fingerprint_key(candidate.bind(py)))
        .collect::<PyResult<_>>()?;
    Ok(newest_fingerprint(&keys, target, model_name)
        .map(|index| candidates[index].bind(py).clone()))
}

impl MigrationStateInspection {
    fn access<'py>(&self, py: Python<'py>) -> PyResult<StateAccess<'py>> {
        let adapter: &Bound<'py, PyAny> = self.adapter.bind(py);
        Ok(StateAccess {
            connection: self.connection.bind(py).clone(),
            execute: native_field(adapter, "execute")?,
            render_qualified_name: native_field(adapter, "render_qualified_name")?,
        })
    }

    fn list_relations<'py>(
        &self,
        py: Python<'py>,
        schemas: Vec<Bound<'py, PyAny>>,
        names: Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        call_with(
            &host_entry(py, LIST_RELATIONS_FOR_INSPECTION)?,
            &[
                ("adapter", self.adapter.bind(py).clone()),
                ("connection", self.connection.bind(py).clone()),
                ("database", self.database.bind(py).clone()),
                ("schemas", PyTuple::new(py, schemas)?.into_any()),
                ("names", names),
            ],
        )?
        .try_iter()?
        .collect()
    }

    fn model_fingerprints<'py>(
        &self,
        py: Python<'py>,
        schema: &Bound<'py, PyAny>,
        key: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let known: Bound<'py, PyAny> = mapping_get(self.known_fingerprints.bind(py), key)?;
        let latest: Bound<'py, PyAny> = if known.is_none() {
            let adapter: &Bound<'py, PyAny> = self.adapter.bind(py);
            let fingerprint_set: Bound<'py, PyAny> = read_latest_fingerprints(Some(&keywords(
                py,
                &[
                    ("connection", self.connection.bind(py).clone()),
                    ("execute", native_field(adapter, "execute")?),
                    ("table_exists", PyBool::new(py, true).to_owned().into_any()),
                    ("database", self.database.bind(py).clone()),
                    ("schema", schema.clone()),
                    (
                        "render_qualified_name",
                        native_field(adapter, "render_qualified_name")?,
                    ),
                    (
                        "render_read_latest_sql",
                        native_field(adapter, "render_read_latest_fingerprints_sql")?,
                    ),
                ],
            )?))?;
            let by_identity: Bound<'py, PyAny> =
                native_field(&fingerprint_set, "fingerprints_by_identity")?;
            if by_identity.is_truthy()? {
                by_identity.call_method0("values")?
            } else {
                PyTuple::empty(py).into_any()
            }
        } else {
            known
        };
        let mut models: Vec<Py<PyAny>> = Vec::new();
        for fingerprint in latest.try_iter()? {
            let fingerprint: Bound<'py, PyAny> = fingerprint?;
            if native_field(&fingerprint, "node_type")?.eq(NODE_TYPE_MODEL)? {
                models.push(fingerprint.unbind());
            }
        }
        Ok(models)
    }
}

#[pymethods]
impl MigrationStateInspection {
    #[new]
    #[pyo3(signature = (*, adapter, connection, database, known_fingerprints = None))]
    fn new(
        adapter: &Bound<'_, PyAny>,
        connection: &Bound<'_, PyAny>,
        database: &Bound<'_, PyAny>,
        known_fingerprints: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let py: Python<'_> = adapter.py();
        let known: Bound<'_, PyAny> = match known_fingerprints {
            Some(known)
                if !known.is_none()
                    && native_field(known, "unfiltered_database")?.eq(database)? =>
            {
                native_field(known, "unfiltered_schemas")?
            }
            _ => PyDict::new(py).into_any(),
        };
        Ok(Self {
            adapter: adapter.clone().unbind(),
            connection: connection.clone().unbind(),
            database: database.clone().unbind(),
            known_fingerprints: known.unbind(),
            read_schemas: HashSet::new(),
            fingerprints: Vec::new(),
            events: Vec::new(),
            event_schemas: HashSet::new(),
            relations: HashMap::new(),
            columns: HashMap::new(),
            column_event_schemas: HashSet::new(),
            column_events: Vec::new(),
            old_name_schemas: HashSet::new(),
            old_name_moves: Vec::new(),
            old_name_facts: Vec::new(),
        })
    }

    /// The database whose namespaces this inspection reads.
    #[getter]
    pub(crate) fn database(&self, py: Python<'_>) -> Py<PyAny> {
        self.database.clone_ref(py)
    }

    /// Every migration event read so far.
    #[getter]
    pub(crate) fn events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        tuple_of(py, &self.events)
    }

    /// Every column migration event read so far.
    #[getter]
    pub(crate) fn column_events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        tuple_of(py, &self.column_events)
    }

    /// The old-name history of every move read by `inspect_old_name_views`.
    #[getter]
    pub(crate) fn old_name_histories<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        old_name_views(
            tuple_of(py, &self.old_name_moves)?.as_any(),
            tuple_of(py, &self.old_name_facts)?.as_any(),
        )
    }

    /// Read the column migration state table once for every schema not inspected yet.
    #[pyo3(signature = (*, schemas))]
    fn inspect_column_events(slf: &Bound<'_, Self>, schemas: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let pending = pending(schemas, &slf.borrow().column_event_schemas)?;
        if pending.is_empty() {
            return Ok(());
        }
        slf.borrow_mut()
            .column_event_schemas
            .extend(pending.iter().map(|(_, key)| key.clone()));
        let listed: Vec<Bound<'_, PyAny>> = slf.borrow().list_relations(
            py,
            pending.iter().map(|(schema, _)| schema.clone()).collect(),
            PyTuple::new(py, [COLUMN_MIGRATION_TABLE_NAME])?.into_any(),
        )?;
        let mut present: HashSet<String> = HashSet::new();
        for relation in &listed {
            if lowered(&native_field(relation, "name")?)? == COLUMN_MIGRATION_TABLE_NAME {
                present.insert(lowered(&schema_or_empty(native_field(
                    relation, "schema",
                )?))?);
            }
        }
        for (schema, key) in pending {
            if !present.contains(&key) {
                continue;
            }
            let (access, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (this.access(py)?, this.database.bind(py).clone())
            };
            let events = read_events(&access, &layout(COLUMN_KIND)?, (database, schema), None)?;
            slf.borrow_mut()
                .column_events
                .extend(events.iter().map(Bound::unbind));
        }
        Ok(())
    }

    /// Read moves and old-name facts once for every schema not inspected yet.
    #[pyo3(signature = (*, schemas))]
    fn inspect_old_name_views(slf: &Bound<'_, Self>, schemas: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let pending = pending(schemas, &slf.borrow().old_name_schemas)?;
        if pending.is_empty() {
            return Ok(());
        }
        slf.borrow_mut()
            .old_name_schemas
            .extend(pending.iter().map(|(_, key)| key.clone()));
        let listed: Vec<Bound<'_, PyAny>> = slf.borrow().list_relations(
            py,
            pending.iter().map(|(schema, _)| schema.clone()).collect(),
            PyTuple::new(py, [MIGRATION_TABLE_NAME, OLD_NAME_VIEW_TABLE_NAME])?.into_any(),
        )?;
        let state_tables: HashSet<RelationKey> =
            listed.iter().map(relation_key).collect::<PyResult<_>>()?;
        for (schema, key) in pending {
            if !state_tables.contains(&(key.clone(), OLD_NAME_VIEW_TABLE_NAME.to_owned()))
                || !state_tables.contains(&(key, MIGRATION_TABLE_NAME.to_owned()))
            {
                continue;
            }
            let (access, adapter, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (
                    this.access(py)?,
                    this.adapter.bind(py).clone(),
                    this.database.bind(py).clone(),
                )
            };
            let stored: Vec<String> = stored_old_name_columns(
                &adapter,
                &access.connection,
                (database.clone(), schema.clone()),
            )?;
            let facts = read_events(
                &access,
                &layout(OLD_NAME_KIND)?,
                (database.clone(), schema.clone()),
                Some(&stored),
            )?;
            slf.borrow_mut()
                .old_name_facts
                .extend(facts.iter().map(Bound::unbind));
            let moves = read_events(&access, &layout(MODEL_KIND)?, (database, schema), None)?;
            slf.borrow_mut()
                .old_name_moves
                .extend(moves.iter().map(Bound::unbind));
        }
        Ok(())
    }

    /// Read migration events once from schemas known to hold the migration table.
    #[pyo3(signature = (*, schemas))]
    fn inspect_migration_events(slf: &Bound<'_, Self>, schemas: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let mut sorted: Vec<(String, Bound<'_, PyAny>)> = Vec::new();
        for schema in schemas.try_iter()? {
            let schema: Bound<'_, PyAny> = schema?;
            sorted.push((schema.extract()?, schema));
        }
        sorted.sort_by(|left, right| left.0.cmp(&right.0));
        for (_, schema) in sorted {
            let key: String = lowered(&schema)?;
            if !slf.borrow_mut().event_schemas.insert(key) {
                continue;
            }
            let (access, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (this.access(py)?, this.database.bind(py).clone())
            };
            let events = read_events(&access, &layout(MODEL_KIND)?, (database, schema), None)?;
            slf.borrow_mut()
                .events
                .extend(events.iter().map(Bound::unbind));
        }
        Ok(())
    }

    /// Read state tables once for every schema not inspected yet.
    #[pyo3(signature = (*, schemas))]
    fn inspect_schemas(slf: &Bound<'_, Self>, schemas: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let pending = pending(schemas, &slf.borrow().read_schemas)?;
        if pending.is_empty() {
            return Ok(());
        }
        slf.borrow_mut()
            .read_schemas
            .extend(pending.iter().map(|(_, key)| key.clone()));
        let listed: Vec<Bound<'_, PyAny>> = slf.borrow().list_relations(
            py,
            pending.iter().map(|(schema, _)| schema.clone()).collect(),
            PyTuple::new(py, [MIGRATION_TABLE_NAME, FINGERPRINT_TABLE_NAME])?.into_any(),
        )?;
        let state_tables: HashSet<RelationKey> =
            listed.iter().map(relation_key).collect::<PyResult<_>>()?;
        for (schema, key) in pending {
            if state_tables.contains(&(key.clone(), MIGRATION_TABLE_NAME.to_owned())) {
                let single = PyTuple::new(py, [schema.clone()])?;
                Self::inspect_migration_events(slf, single.as_any())?;
            }
            if state_tables.contains(&(key.clone(), FINGERPRINT_TABLE_NAME.to_owned())) {
                let models: Vec<Py<PyAny>> = slf.borrow().model_fingerprints(py, &schema, &key)?;
                let mut this: PyRefMut<'_, Self> = slf.borrow_mut();
                match this
                    .fingerprints
                    .iter_mut()
                    .find(|(stored, _)| *stored == key)
                {
                    Some((_, stored)) => *stored = models,
                    None => this.fingerprints.push((key, models)),
                }
            }
        }
        Ok(())
    }

    /// List candidate relations and their columns with one metadata read.
    #[pyo3(signature = (*, locations))]
    fn inspect_relations(slf: &Bound<'_, Self>, locations: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let mut wanted: HashSet<RelationKey> = HashSet::new();
        let mut schemas: Vec<String> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for location in locations.try_iter()? {
            let location: Bound<'_, PyAny> = location?;
            if native_field(&location, "schema")?.is_none() {
                continue;
            }
            let key: RelationKey = relation_key(&location)?;
            if slf.borrow().relations.contains_key(&key) {
                continue;
            }
            wanted.insert(key);
            schemas.push(native_field(&location, "schema")?.extract()?);
            names.push(native_field(&location, "name")?.extract()?);
        }
        if wanted.is_empty() {
            return Ok(());
        }
        schemas.sort();
        schemas.dedup();
        names.sort();
        names.dedup();
        let listed: Vec<Bound<'_, PyAny>> = slf.borrow().list_relations(
            py,
            schemas
                .iter()
                .map(|schema| PyString::new(py, schema).into_any())
                .collect(),
            PyTuple::new(py, &names)?.into_any(),
        )?;
        let mut matched: Vec<(RelationKey, Bound<'_, PyAny>)> = Vec::new();
        for relation in listed {
            let key: RelationKey = relation_key(&relation)?;
            if wanted.contains(&key) {
                matched.push((key, relation));
            }
        }
        let columns: Bound<'_, PyAny> = if matched.is_empty() {
            PyDict::new(py).into_any()
        } else {
            let this: PyRef<'_, Self> = slf.borrow();
            let adapter: Bound<'_, PyAny> = this.adapter.bind(py).clone();
            let connection: Bound<'_, PyAny> = this.connection.bind(py).clone();
            drop(this);
            call_with(
                &host_entry(py, GET_COLUMNS_FOR_INSPECTION)?,
                &[
                    ("adapter", adapter),
                    ("connection", connection),
                    (
                        "relations",
                        PyTuple::new(py, matched.iter().map(|(_, relation)| relation))?.into_any(),
                    ),
                ],
            )?
        };
        let empty: Bound<'_, PyTuple> = PyTuple::empty(py);
        for (key, relation) in matched {
            let relation_columns: Bound<'_, PyAny> =
                mapping_get_or(&columns, native_field(&relation, "identity")?, &empty)?;
            let mut this: PyRefMut<'_, Self> = slf.borrow_mut();
            this.relations.insert(key.clone(), relation.unbind());
            this.columns.insert(key, relation_columns.unbind());
        }
        Ok(())
    }

    /// The listed relation for a location.
    fn relation(&self, py: Python<'_>, location: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Ok(match self.relations.get(&relation_key(location)?) {
            Some(relation) => relation.clone_ref(py),
            None => py.None(),
        })
    }

    /// Listed columns for a location.
    fn relation_columns(&self, py: Python<'_>, location: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Ok(match self.columns.get(&relation_key(location)?) {
            Some(columns) => columns.clone_ref(py),
            None => PyTuple::empty(py).into_any().unbind(),
        })
    }

    /// The latest model fingerprint that built the location or named model.
    #[pyo3(signature = (*, location, model_name))]
    fn model_fingerprint<'py>(
        &self,
        py: Python<'py>,
        location: &Bound<'py, PyAny>,
        model_name: Option<String>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let key: String = lowered(&schema_or_empty(native_field(location, "schema")?))?;
        let Some((_, candidates)) = self.fingerprints.iter().find(|(stored, _)| *stored == key)
        else {
            return Ok(None);
        };
        let target = EventRelation {
            database: native_field(location, "database")?.extract()?,
            schema: native_field(location, "schema")?.extract()?,
            name: native_field(location, "name")?.extract()?,
        };
        newest(py, candidates, (Some(&target), model_name.as_deref()))
    }

    /// The newest model fingerprint with this node name in any inspected schema.
    #[pyo3(signature = (*, model_name))]
    fn named_fingerprint<'py>(
        &self,
        py: Python<'py>,
        model_name: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut all: Vec<Py<PyAny>> = Vec::new();
        for (_, fingerprints) in &self.fingerprints {
            all.extend(fingerprints.iter().map(|item| item.clone_ref(py)));
        }
        newest(py, &all, (None, Some(model_name)))
    }

    /// Latest model fingerprints whose names are no longer in the project.
    #[pyo3(signature = (*, project_model_names))]
    fn orphan_fingerprints<'py>(
        &self,
        py: Python<'py>,
        project_model_names: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let mut orphans: Vec<Bound<'py, PyAny>> = Vec::new();
        for (_, fingerprints) in &self.fingerprints {
            for fingerprint in fingerprints {
                let fingerprint: &Bound<'py, PyAny> = fingerprint.bind(py);
                if !project_model_names.contains(native_field(fingerprint, "node_name")?)? {
                    orphans.push(fingerprint.clone());
                }
            }
        }
        PyTuple::new(py, orphans)
    }
}

/// A fresh live-state inspection over the runtime's connection and planning database.
pub(super) fn inspection<'py>(
    runtime: &Bound<'py, PyAny>,
    known_fingerprints: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let state = MigrationStateInspection::new(
        &runtime_field(runtime, |runtime| &runtime.adapter)?,
        &runtime_field(runtime, |runtime| &runtime.connection)?,
        &super::planning_origin::planning_database(runtime)?,
        known_fingerprints,
    )?;
    Ok(Bound::new(runtime.py(), state)?.into_any())
}
