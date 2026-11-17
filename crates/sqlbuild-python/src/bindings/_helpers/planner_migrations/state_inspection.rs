//! Read-only warehouse evidence gathered once for all model migrations in one plan.

use crate::bindings::_helpers::planner_warehouse::snapshot::{columns_for, list_relations};
use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods,
};
use pyo3::types::{PyDict, PyString, PyTuple, PyTupleMethods};
use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_state::events::main::newest_fingerprint::newest_fingerprint;
use sqlbuild_state::events::models::{EventRelation, FingerprintKey, FingerprintOrder};

use crate::bindings::_helpers::planner_phases::runtime::runtime_reads;
use crate::bindings::_helpers::planner_warehouse::reads::FingerprintRead;
use crate::bindings::_helpers::planner_warehouse::reads::PlannerReads;
use crate::bindings::_helpers::planner_warehouse::reads::Read;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use crate::bindings::_helpers::state_rows::constants::COLUMN_KIND;
use crate::bindings::_helpers::state_rows::constants::MODEL_KIND;
use crate::bindings::_helpers::state_rows::constants::OLD_NAME_KIND;
use crate::bindings::_helpers::state_rows::migration_objects::layout;
use crate::bindings::_helpers::state_rows::migration_projections::old_name_views;
use crate::bindings::_helpers::planner_migrations::constants::GET_COLUMNS_FOR_INSPECTION;
use crate::bindings::_helpers::planner_migrations::constants::LIST_RELATIONS_FOR_INSPECTION;
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_MODEL;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_front::state_rows::constants::COLUMN_KIND;
use sqlbuild_python_front::state_rows::constants::MODEL_KIND;
use sqlbuild_python_front::state_rows::constants::OLD_NAME_KIND;
use sqlbuild_python_front::state_rows::main::migrations::layout::layout;
use sqlbuild_python_front::state_rows::main::migrations::old_name_views::old_name_views;
use sqlbuild_python_front::state_rows::main::migrations::read_events::read_events;
use sqlbuild_python_front::state_rows::main::migrations::stored_old_name_columns::stored_old_name_columns;
use sqlbuild_python_front::state_rows::models::StateAccess;

/// A lowercased schema and relation name.
type RelationKey = (String, String);

/// Relations, fingerprints and migration events for one planning database.
#[pyclass(module = "sqlbuild._native", name = "MigrationStateInspection")]
pub(crate) struct MigrationStateInspection {
    reads: Py<PlannerReads>,
    database: Py<PyAny>,
    known_fingerprints: Py<PyAny>,
    read_schemas: HashSet<String>,
    /// Model fingerprints per lowercased schema, in first-read order.
    fingerprints: Vec<(String, Vec<Py<PyAny>>)>,
    /// Positions in `fingerprints` (schema, fingerprint) by node name, in the order `fingerprints` lists them.
    fingerprints_by_name: HashMap<String, Vec<(usize, usize)>>,
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
    fn reads<'py>(&self, py: Python<'py>) -> Bound<'py, PlannerReads> {
        self.reads.bind(py).clone()
    }

    fn list_relations<'py>(
        &self,
        py: Python<'py>,
        schemas: Vec<Bound<'py, PyAny>>,
        names: Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        list_relations(
            self.reads.bind(py),
            self.database.bind(py),
            PyTuple::new(py, schemas)?.into_any(),
            names,
        )?
        .try_iter()?
        .collect()
    }

    /// The listed migration state tables of `schemas`, or `None` when the reads do not know them.
    fn migration_state_tables<'py>(
        &self,
        py: Python<'py>,
        schemas: Vec<Bound<'py, PyAny>>,
        names: Bound<'py, PyAny>,
    ) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        match self.reads.get().reads().migration_state_tables(
            py,
            self.database.bind(py),
            (PyTuple::new(py, schemas)?.into_any(), names),
        )? {
            Read::Known(listed) => Ok(Some(listed.try_iter()?.collect::<PyResult<_>>()?)),
            Read::Unknown(_) => Ok(None),
        }
    }

    /// Every inspected schema's model fingerprint positions by node name, in the order `fingerprints` lists them.
    fn by_name(&self, py: Python<'_>) -> PyResult<HashMap<String, Vec<(usize, usize)>>> {
        let mut by_name: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for (schema, (_, fingerprints)) in self.fingerprints.iter().enumerate() {
            for (position, fingerprint) in fingerprints.iter().enumerate() {
                let name: String = native_field(fingerprint.bind(py), "node_name")?.extract()?;
                by_name.entry(name).or_default().push((schema, position));
            }
        }
        Ok(by_name)
    }

    fn model_fingerprints<'py>(
        &self,
        py: Python<'py>,
        schema: &Bound<'py, PyAny>,
        key: &str,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let known: Bound<'py, PyAny> = mapping_get(self.known_fingerprints.bind(py), key)?;
        let latest: Bound<'py, PyAny> = if known.is_none() {
            let schema: String = schema.extract()?;
            let fingerprint_set: Bound<'py, PyAny> = self.reads.get().reads().fingerprints(
                py,
                FingerprintRead {
                    database: self.database.bind(py).extract()?,
                    schema: &schema,
                    table_exists: true,
                    node_names: None,
                    filtered_node_types: Vec::new(),
                },
            )?;
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
    #[pyo3(signature = (*, reads, database, known_fingerprints = None))]
    fn new(
        reads: &Bound<'_, PlannerReads>,
        database: &Bound<'_, PyAny>,
        known_fingerprints: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let py: Python<'_> = reads.py();
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
            reads: reads.clone().unbind(),
            database: database.clone().unbind(),
            known_fingerprints: known.unbind(),
            read_schemas: HashSet::new(),
            fingerprints: Vec::new(),
            fingerprints_by_name: HashMap::new(),
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
        let Some(listed) = slf.borrow().migration_state_tables(
            py,
            pending.iter().map(|(schema, _)| schema.clone()).collect(),
            PyTuple::new(py, [COLUMN_MIGRATION_TABLE_NAME])?.into_any(),
        )?
        else {
            return Ok(());
        };
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
            let (reads, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (this.reads(py), this.database.bind(py).clone())
            };
            let Read::Known(events) = reads.get().reads().recorded_migrations(
                py,
                &layout(COLUMN_KIND)?,
                (database, schema),
                None,
            )?
            else {
                return Ok(());
            };
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
        let Some(listed) = slf.borrow().migration_state_tables(
            py,
            pending.iter().map(|(schema, _)| schema.clone()).collect(),
            PyTuple::new(py, [MIGRATION_TABLE_NAME, OLD_NAME_VIEW_TABLE_NAME])?.into_any(),
        )?
        else {
            return Ok(());
        };
        let state_tables: HashSet<RelationKey> =
            listed.iter().map(relation_key).collect::<PyResult<_>>()?;
        for (schema, key) in pending {
            if !state_tables.contains(&(key.clone(), OLD_NAME_VIEW_TABLE_NAME.to_owned()))
                || !state_tables.contains(&(key, MIGRATION_TABLE_NAME.to_owned()))
            {
                continue;
            }
            let (reads, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (this.reads(py), this.database.bind(py).clone())
            };
            let reads = reads.get().reads();
            let Read::Known(stored) =
                reads.old_name_view_columns(py, (database.clone(), schema.clone()))?
            else {
                return Ok(());
            };
            let Read::Known(facts) = reads.recorded_migrations(
                py,
                &layout(OLD_NAME_KIND)?,
                (database.clone(), schema.clone()),
                Some(&stored),
            )?
            else {
                return Ok(());
            };
            slf.borrow_mut()
                .old_name_facts
                .extend(facts.iter().map(Bound::unbind));
            let Read::Known(moves) =
                reads.recorded_migrations(py, &layout(MODEL_KIND)?, (database, schema), None)?
            else {
                return Ok(());
            };
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
            let (reads, database) = {
                let this: PyRef<'_, Self> = slf.borrow();
                (this.reads(py), this.database.bind(py).clone())
            };
            let Read::Known(events) = reads.get().reads().recorded_migrations(
                py,
                &layout(MODEL_KIND)?,
                (database, schema),
                None,
            )?
            else {
                return Ok(());
            };
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
        let mut read_fingerprints: bool = false;
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
                read_fingerprints = true;
            }
        }
        if read_fingerprints {
            let by_name: HashMap<String, Vec<(usize, usize)>> = slf.borrow().by_name(py)?;
            slf.borrow_mut().fingerprints_by_name = by_name;
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
            let reads: Bound<'_, PlannerReads> = slf.borrow().reads(py);
            columns_for(
                &reads,
                PyTuple::new(py, matched.iter().map(|(_, relation)| relation))?.into_any(),
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
        let named: Vec<Py<PyAny>> = self
            .fingerprints_by_name
            .get(model_name)
            .into_iter()
            .flatten()
            .filter_map(|(schema, position)| {
                self.fingerprints
                    .get(*schema)
                    .and_then(|(_, fingerprints)| fingerprints.get(*position))
            })
            .map(|fingerprint| fingerprint.clone_ref(py))
            .collect();
        newest(py, &named, (None, Some(model_name)))
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
        &runtime_reads(runtime)?,
        &super::planning_origin::planning_database(runtime)?,
        known_fingerprints,
    )?;
    Ok(Bound::new(runtime.py(), state)?.into_any())
}
