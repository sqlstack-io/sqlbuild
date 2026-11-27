//! Public records of planner migrations.

use crate::planner_migrations::_helpers::discovery::state_inspection::RelationKey;
use crate::planner_migrations::_helpers::discovery::state_inspection::fingerprint_key;
use crate::planner_migrations::_helpers::discovery::state_inspection::lowered;
use crate::planner_migrations::_helpers::discovery::state_inspection::pending;
use crate::planner_migrations::_helpers::discovery::state_inspection::relation_key;
use crate::planner_migrations::_helpers::discovery::state_inspection::schema_or_empty;
use crate::planner_migrations::_helpers::discovery::state_inspection::tuple_of;
use crate::planner_migrations::_helpers::old_name::identity_index::IdentityIndex;
use crate::planner_warehouse::main::inspection::columns_for::columns_for;
use crate::planner_warehouse::main::inspection::list_relations::list_relations;
use crate::planner_warehouse::models::FingerprintRead;
use crate::planner_warehouse::models::PlannerReads;
use crate::planner_warehouse::models::Read;
use crate::planner_warehouse::models::StateHistoryRead;
use pyo3::prelude::Py;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::prelude::{Bound, PyAny};
use pyo3::types::PyDict;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_MODEL;
use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_front::state_rows::constants::COLUMN_KIND;
use sqlbuild_python_front::state_rows::constants::MODEL_KIND;
use sqlbuild_python_front::state_rows::constants::OLD_NAME_KIND;
use sqlbuild_python_front::state_rows::main::migrations::layout::layout;
use sqlbuild_python_front::state_rows::main::migrations::old_name_views::old_name_views;
use sqlbuild_state::events::main::newest_fingerprint::newest_fingerprint;
use sqlbuild_state::events::models::{EventRelation, FingerprintKey};
use std::cell::OnceCell;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug)]
pub(crate) struct OldNamePlanner<'py> {
    pub runtime: Bound<'py, PyAny>,
    pub models: Vec<Bound<'py, PyAny>>,
    pub histories: Vec<Bound<'py, PyAny>>,
    pub views: Bound<'py, PyAny>,
    pub columns: Bound<'py, PyAny>,
    pub retention: Bound<'py, PyAny>,
    pub now: Bound<'py, PyAny>,
    /// `models` by destination relation, built on first use.
    pub model_index: OnceCell<IdentityIndex>,
    /// `views` by old relation, built on first use.
    pub view_index: OnceCell<IdentityIndex>,
    /// `histories` by their move's destination, built on first use.
    pub history_index: OnceCell<IdentityIndex>,
}

#[derive(Debug)]
pub(crate) struct ColumnCandidate<'py> {
    pub model: Bound<'py, PyAny>,
    pub warehouse: Bound<'py, PyDict>,
    pub declared: Bound<'py, PyDict>,
    pub fingerprint: Bound<'py, PyAny>,
    pub previous: Bound<'py, PyAny>,
    pub current: Bound<'py, PyAny>,
}

#[derive(Debug)]
pub(crate) struct CheckedColumns<'py> {
    pub entries: Vec<Bound<'py, PyAny>>,
    pub warnings: Vec<Bound<'py, PyAny>>,
}

/// The planning-phase inputs every migration planner reads.
#[derive(Debug)]
pub struct MigrationPhase<'py> {
    pub runtime: Bound<'py, PyAny>,
    pub scope: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub overrides: Bound<'py, PyAny>,
    pub deferral: Bound<'py, PyAny>,
}

/// The column migration planning inputs beyond the phase.
#[derive(Debug)]
pub struct ColumnMigrationInputs<'py> {
    pub full_refresh_model_names: Bound<'py, PyAny>,
    pub physical_relations: Bound<'py, PyAny>,
    pub source_columns: Bound<'py, PyAny>,
}

/// The snapshot after model migrations overlay it, with the planned entries and their warnings.
#[derive(Debug)]
pub struct ModelMigrationPlanning<'py> {
    pub snapshot: Bound<'py, PyAny>,
    pub entries: Vec<Bound<'py, PyAny>>,
    pub warnings: Vec<Bound<'py, PyAny>>,
}

/// The snapshot after column renames overlay it, with the planned entries, rename hints and warnings.
#[derive(Debug)]
pub struct ColumnMigrationPlanning<'py> {
    pub snapshot: Bound<'py, PyAny>,
    pub entries: Vec<Bound<'py, PyAny>>,
    pub hints: Vec<Bound<'py, PyAny>>,
    pub warnings: Vec<Bound<'py, PyAny>>,
}

/// The old-name view steps, the recorded live views and the warnings they raise.
#[derive(Debug)]
pub struct OldNameViewPlanning<'py> {
    pub entries: Bound<'py, PyAny>,
    pub views: Bound<'py, PyAny>,
    pub warnings: Vec<Bound<'py, PyAny>>,
}

/// One destination model whose data should come from an earlier relation.
#[derive(Debug, Clone)]
pub(crate) struct ModelMigrationRequest<'py> {
    pub model: Bound<'py, PyAny>,
    pub discovery: Bound<'py, PyAny>,
    pub raw_origin: Option<Bound<'py, PyAny>>,
    pub origin_location: Option<Bound<'py, PyAny>>,
    pub origin_model: Bound<'py, PyAny>,
    pub force: bool,
    pub identity_only: bool,
}

/// One project model's explicit migrate_from, resolved to a relation.
#[derive(Debug)]
pub(crate) struct ModelMigrationDeclaration<'py> {
    pub model_name: Bound<'py, PyAny>,
    pub origin_location: Bound<'py, PyAny>,
    pub origin_model: Bound<'py, PyAny>,
}

/// Manual and automatically discovered migration requests for one plan.
#[derive(Debug)]
pub(crate) struct ModelMigrationDiscovery<'py> {
    pub requests: Vec<ModelMigrationRequest<'py>>,
    pub warnings: Vec<Bound<'py, PyAny>>,
    pub destination_fingerprints: Bound<'py, PyDict>,
}

/// Compatibility status plus human-readable blocking findings.
#[derive(Debug)]
pub(crate) struct MigrationCompatibilityResult<'py> {
    pub status: Bound<'py, PyAny>,
    pub findings: Bound<'py, PyAny>,
}

/// The fingerprint inputs shared by migration matching and destination fingerprints.
#[derive(Debug)]
pub(crate) struct FingerprintInputs<'py> {
    pub metadata_jsons: Bound<'py, PyAny>,
    pub dialect: Bound<'py, PyAny>,
    pub fingerprints: Bound<'py, PyAny>,
}

/// One resolved migration request decided against live state.
#[derive(Debug)]
pub(crate) struct MigrationDecisionRequest<'a, 'py> {
    pub runtime: Bound<'py, PyAny>,
    pub request: &'a ModelMigrationRequest<'py>,
    pub origin: Bound<'py, PyAny>,
    pub origin_model: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub state: Bound<'py, PyAny>,
    pub snapshots_config: Bound<'py, PyAny>,
}

/// What checking an origin relation's compatibility with its destination model reads.
#[derive(Debug)]
pub(crate) struct CompatibilityCheck<'py> {
    pub model: Bound<'py, PyAny>,
    pub origin_relation: Bound<'py, PyAny>,
    pub origin_columns: Bound<'py, PyAny>,
    pub sql_analysis_enabled: Bound<'py, PyAny>,
    pub column_dialect: Bound<'py, PyAny>,
    pub snapshots_config: Bound<'py, PyAny>,
}

/// What discovering automatic and manual migration requests reads.
#[derive(Debug)]
pub(crate) struct DiscoveryRequest<'py> {
    pub runtime: Bound<'py, PyAny>,
    pub scope: Bound<'py, PyAny>,
    pub snapshot: Bound<'py, PyAny>,
    pub manual_requests: Vec<ModelMigrationRequest<'py>>,
    pub declarations: Vec<ModelMigrationDeclaration<'py>>,
    pub state: Bound<'py, PyAny>,
    pub project_schemas: Bound<'py, PyAny>,
    pub fingerprints: Bound<'py, PyAny>,
}

/// One schema's fingerprint positions by lowercased built relation name and by node name.
#[derive(Debug, Default)]
struct FingerprintPositions {
    by_target: HashMap<String, Vec<usize>>,
    by_node: HashMap<String, Vec<usize>>,
}

impl FingerprintPositions {
    fn of(keys: &[FingerprintKey]) -> Self {
        let mut positions: Self = Self::default();
        for (position, key) in keys.iter().enumerate() {
            if let Some(target) = &key.target {
                positions
                    .by_target
                    .entry(target.name.to_lowercase())
                    .or_default()
                    .push(position);
            }
            positions
                .by_node
                .entry(key.node_name.clone())
                .or_default()
                .push(position);
        }
        positions
    }

    /// Positions that built a relation named `target` or ran as `model_name`, in stored order.
    fn candidates(&self, target: &str, model_name: Option<&str>) -> Vec<usize> {
        let mut found: Vec<usize> = self
            .by_target
            .get(&target.to_lowercase())
            .into_iter()
            .flatten()
            .chain(
                model_name
                    .and_then(|name| self.by_node.get(name))
                    .into_iter()
                    .flatten(),
            )
            .copied()
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// Relations, fingerprints and migration events for one planning database.
#[pyclass(module = "sqlbuild._native", name = "MigrationStateInspection")]
#[derive(Debug)]
pub struct MigrationStateInspection {
    reads: Py<PlannerReads>,
    database: Py<PyAny>,
    known_fingerprints: Py<PyAny>,
    read_schemas: HashSet<String>,
    /// Model fingerprints per lowercased schema, in first-read order.
    fingerprints: Vec<(String, Vec<Py<PyAny>>)>,
    /// Each `fingerprints` entry's lookup keys, read from the fingerprints once.
    fingerprint_keys: Vec<Vec<FingerprintKey>>,
    /// Each `fingerprints` entry's positions by lowercased built relation name and by node name.
    fingerprint_positions: Vec<FingerprintPositions>,
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
        match self.reads.get().reads().state_history(
            py,
            StateHistoryRead::MigrationTables {
                database: self.database.bind(py),
                filter: (PyTuple::new(py, schemas)?.into_any(), names),
            },
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
    pub(crate) fn new(
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
            fingerprint_keys: Vec::new(),
            fingerprint_positions: Vec::new(),
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
            let Read::Known(events) = reads.get().reads().state_history(
                py,
                StateHistoryRead::MigrationEvents {
                    layout: &layout(COLUMN_KIND)?,
                    location: (database, schema),
                    stored: None,
                },
            )?
            else {
                return Ok(());
            };
            slf.borrow_mut()
                .column_events
                .extend(events.cast::<PyTuple>()?.iter().map(Bound::unbind));
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
            let Read::Known(facts) = reads.state_history(
                py,
                StateHistoryRead::MigrationEvents {
                    layout: &layout(OLD_NAME_KIND)?,
                    location: (database.clone(), schema.clone()),
                    stored: Some(&stored),
                },
            )?
            else {
                return Ok(());
            };
            slf.borrow_mut()
                .old_name_facts
                .extend(facts.cast::<PyTuple>()?.iter().map(Bound::unbind));
            let Read::Known(moves) = reads.state_history(
                py,
                StateHistoryRead::MigrationEvents {
                    layout: &layout(MODEL_KIND)?,
                    location: (database, schema),
                    stored: None,
                },
            )?
            else {
                return Ok(());
            };
            slf.borrow_mut()
                .old_name_moves
                .extend(moves.cast::<PyTuple>()?.iter().map(Bound::unbind));
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
            let Read::Known(events) = reads.get().reads().state_history(
                py,
                StateHistoryRead::MigrationEvents {
                    layout: &layout(MODEL_KIND)?,
                    location: (database, schema),
                    stored: None,
                },
            )?
            else {
                return Ok(());
            };
            slf.borrow_mut()
                .events
                .extend(events.cast::<PyTuple>()?.iter().map(Bound::unbind));
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
                let keys: Vec<FingerprintKey> = models
                    .iter()
                    .map(|model| fingerprint_key(model.bind(py)))
                    .collect::<PyResult<_>>()?;
                let mut this: PyRefMut<'_, Self> = slf.borrow_mut();
                match this
                    .fingerprints
                    .iter()
                    .position(|(stored, _)| *stored == key)
                {
                    Some(index) => {
                        this.fingerprints[index].1 = models;
                        this.fingerprint_positions[index] = FingerprintPositions::of(&keys);
                        this.fingerprint_keys[index] = keys;
                    }
                    None => {
                        this.fingerprints.push((key, models));
                        this.fingerprint_positions
                            .push(FingerprintPositions::of(&keys));
                        this.fingerprint_keys.push(keys);
                    }
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
        let Some(index) = self
            .fingerprints
            .iter()
            .position(|(stored, _)| *stored == key)
        else {
            return Ok(None);
        };
        let target = EventRelation {
            database: native_field(location, "database")?.extract()?,
            schema: native_field(location, "schema")?.extract()?,
            name: native_field(location, "name")?.extract()?,
        };
        let positions: Vec<usize> =
            self.fingerprint_positions[index].candidates(&target.name, model_name.as_deref());
        let keys: Vec<FingerprintKey> = positions
            .iter()
            .map(|&position| self.fingerprint_keys[index][position].clone())
            .collect();
        Ok(
            newest_fingerprint(&keys, Some(&target), model_name.as_deref()).map(|candidate| {
                self.fingerprints[index].1[positions[candidate]]
                    .bind(py)
                    .clone()
            }),
        )
    }

    /// The newest model fingerprint with this node name in any inspected schema.
    #[pyo3(signature = (*, model_name))]
    fn named_fingerprint<'py>(
        &self,
        py: Python<'py>,
        model_name: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let named: Vec<(&Py<PyAny>, FingerprintKey)> = self
            .fingerprints_by_name
            .get(model_name)
            .into_iter()
            .flatten()
            .filter_map(|&(schema, position)| {
                Some((
                    self.fingerprints.get(schema)?.1.get(position)?,
                    self.fingerprint_keys.get(schema)?.get(position)?.clone(),
                ))
            })
            .collect();
        let keys: Vec<FingerprintKey> = named.iter().map(|(_, key)| key.clone()).collect();
        Ok(newest_fingerprint(&keys, None, Some(model_name))
            .map(|position| named[position].0.bind(py).clone()))
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
