//! Assemble new and resumed old-name view steps from recorded facts.

use crate::planner_migrations::_helpers::column::column_objects::model;
use crate::planner_migrations::_helpers::column::column_objects::text;
use crate::planner_migrations::_helpers::old_name::identity_index::IdentityIndex;
use crate::planner_migrations::_helpers::old_name::old_name_objects::expires;
use crate::planner_migrations::_helpers::old_name::old_name_objects::location;
use crate::planner_migrations::_helpers::old_name::old_name_objects::relation;
use crate::planner_migrations::_helpers::old_name::old_name_objects::retention;
use crate::planner_migrations::_helpers::old_name::old_name_objects::stores_history;
use crate::planner_migrations::_helpers::old_name::old_names::declared_column_aliases;
use crate::planner_migrations::constants::PENDING_ARCHIVE;
use crate::planner_migrations::constants::PENDING_VIEW;
use crate::planner_migrations::models::OldNamePlanner;
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use crate::planner_phases::main::inputs::runtime_field::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyBool, PyList, PyTuple};
use sqlbuild_analysis::planner_migrations::main::old_name_skip_reason::old_name_skip_reason;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::planner_phases::main::object::object;
use std::cell::OnceCell;

impl<'py> OldNamePlanner<'py> {
    pub(super) fn new(
        (runtime, columns): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        models: Vec<Bound<'py, PyAny>>,
        histories: Vec<Bound<'py, PyAny>>,
        views: Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let runtime = runtime.clone();
        let py = runtime.py();
        let project_config = runtime_field(&runtime, |runtime| &runtime.project_config)?;
        let config = if project_config.is_none() {
            new_record(py, RecordId::MigrationsConfig, (), None)?
        } else {
            native_field(&project_config, "migrations")?
        };
        Ok(Self {
            models,
            histories,
            views,
            columns: columns.clone(),
            retention: native_field(&config, "old_name_views")?,
            now: runtime_field(&runtime, |runtime| &runtime.invocation_time)?,
            runtime,
            model_index: OnceCell::new(),
            view_index: OnceCell::new(),
            history_index: OnceCell::new(),
        })
    }

    /// `cell`'s index, building it from `relations` the first time.
    fn index<'a>(
        cell: &'a OnceCell<IdentityIndex>,
        relations: impl FnOnce() -> PyResult<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<&'a IdentityIndex> {
        if let Some(index) = cell.get() {
            return Ok(index);
        }
        let built: IdentityIndex = IdentityIndex::of(&relations()?)?;
        Ok(cell.get_or_init(|| built))
    }

    /// Model positions by destination relation.
    fn model_index(&self) -> PyResult<&IdentityIndex> {
        Self::index(&self.model_index, || {
            self.models
                .iter()
                .map(|item| relation(&destination(item)?))
                .collect()
        })
    }

    /// Recorded view positions by old relation.
    fn view_index(&self) -> PyResult<&IdentityIndex> {
        Self::index(&self.view_index, || {
            self.views
                .try_iter()?
                .map(|view| relation(&native_field(&view?, "old")?))
                .collect()
        })
    }

    /// History positions by their move's destination relation.
    fn history_index(&self) -> PyResult<&IdentityIndex> {
        Self::index(&self.history_index, || {
            self.histories
                .iter()
                .map(|history| native_field(&native_field(history, "move")?, "destination"))
                .collect()
        })
    }

    fn newest(
        &self,
        destination: &Bound<'py, PyAny>,
        origin: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut newest: Option<(Bound<'py, PyTuple>, Bound<'py, PyAny>)> = None;
        for position in self.history_index()?.matching(destination)? {
            let history = &self.histories[position];
            let movement = native_field(history, "move")?;
            if let Some(origin) = origin
                && !native_field(&movement, "origin")?
                    .call_method1("matches", (origin,))?
                    .is_truthy()?
            {
                continue;
            }
            let key = PyTuple::new(
                self.runtime.py(),
                [
                    native_field(&movement, "created_at")?,
                    native_field(&movement, "event_id")?,
                ],
            )?;
            if let Some((previous, _)) = &newest
                && !key.gt(previous)?
            {
                continue;
            }
            newest = Some((key, history.clone()));
        }
        Ok(newest.map(|(_, history)| history))
    }

    pub(super) fn for_migration(
        &self,
        entry: &Bound<'py, PyAny>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        if native_field(entry, "blocks_build")?.is_truthy()? {
            return Ok(None);
        }
        let decision = native_field(entry, "decision")?;
        let completed = !native_field(entry, "completed_at")?.is_none();
        if native_field(&decision, "moves_data")?.is_truthy()?
            || (decision.eq("renamed")? && !completed)
        {
            return self.new_move(entry, item).map(Some);
        }
        if decision.eq("done")? || (decision.eq("renamed")? && completed) {
            let origin = relation(&native_field(entry, "origin")?)?;
            if let Some(history) = self.newest(
                &relation(&native_field(entry, "destination")?)?,
                Some(&origin),
            )? {
                return self.resume(&history, item);
            }
        }
        Ok(None)
    }

    pub(super) fn pending_for(
        &self,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(history) = self.newest(&relation(&destination(item)?)?, None)? else {
            return Ok(None);
        };
        if !native_field(&native_field(&history, "move")?, "destination_model")?
            .eq(native_field(item, "name")?)?
        {
            return Ok(None);
        }
        let Some(planned) = self.resume(&history, item)? else {
            return Ok(None);
        };
        if native_field(&planned, "runs_steps")?.is_truthy()?
            || (native_field(&planned, "action")?.eq("none")?
                && !native_field(&planned, "migration_event_id")?.is_none())
        {
            Ok(Some(planned))
        } else {
            Ok(None)
        }
    }

    /// Why no old-name view is kept for `origin`: a model reuses its name or a view already serves it.
    fn skip(
        &self,
        origin: &Bound<'py, PyAny>,
        tracked: Bound<'py, PyAny>,
        retention: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self.runtime.py();
        if retention.is_none() {
            return object(py, old_name_skip_reason(false, None, false, None));
        }
        let old = relation(origin)?;
        let reuser: Option<String> = match self.model_index()?.matching(&old)?.first() {
            Some(position) => Some(name_object(&self.models[*position])?.extract()?),
            None => None,
        };
        let compatibility_view: bool =
            reuser.is_none() && !self.view_index()?.matching(&old)?.is_empty();
        object(
            py,
            old_name_skip_reason(
                true,
                reuser.as_deref(),
                compatibility_view,
                tracked.extract()?,
            ),
        )
    }

    fn aliases(&self, item: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = self.runtime.py();
        if !stores_history(item)? {
            return Ok(declared_column_aliases(py, item)?.into_any());
        }
        let mut aliases: Vec<Bound<'py, PyTuple>> = Vec::new();
        for entry in self.columns.try_iter()? {
            let entry = entry?;
            let decision = native_field(&entry, "decision")?;
            if native_field(&entry, "model_name")?.eq(native_field(item, "name")?)?
                && (decision.eq("rename")? || decision.eq("record")?)
            {
                aliases.push(PyTuple::new(
                    py,
                    [
                        native_field(&entry, "origin_column")?,
                        native_field(&entry, "destination_column")?,
                    ],
                )?);
            }
        }
        let aliases = PyList::new(py, aliases)?;
        aliases.call_method0("sort")?;
        imported(py, "builtins", "tuple")?.call1((aliases,))
    }

    fn new_move(
        &self,
        entry: &Bound<'py, PyAny>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self.runtime.py();
        let retention = retention(item, &self.retention)?;
        let reason = self.skip(
            &native_field(entry, "origin")?,
            native_field(entry, "origin_tracked")?,
            &retention,
        )?;
        let mut fields = vec![
            ("model_name", native_field(entry, "model_name")?),
            ("origin", native_field(entry, "origin")?),
            ("destination", native_field(entry, "destination")?),
            ("target_name", native_field(entry, "target_name")?),
        ];
        if !reason.is_none() || retention.is_none() {
            fields.extend([
                ("action", action(&self.runtime, "NONE")?),
                (
                    "reason",
                    if reason.is_truthy()? {
                        reason
                    } else {
                        text(py, "old_name_view false")
                    },
                ),
            ]);
        } else {
            fields.extend([
                ("action", action(&self.runtime, "ARCHIVE_AND_VIEW")?),
                ("expires_at", expires(&retention, &self.now)?),
                ("retention", retention),
                ("column_aliases", self.aliases(item)?),
                (
                    "stores_history",
                    PyBool::new(py, stores_history(item)?).to_owned().into_any(),
                ),
                (
                    "records_requirement",
                    PyBool::new(py, true).to_owned().into_any(),
                ),
                (
                    "grants_supported",
                    runtime_dialect(&self.runtime)?.syntax("relation_grants_supported")?,
                ),
            ]);
        }
        model(py, RecordId::OldNameViewPlanEntry, &fields)
    }

    fn resume(
        &self,
        history: &Bound<'py, PyAny>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = self.runtime.py();
        let status: String = call_with(
            &native_field(history, "status")?,
            &[("now", self.now.clone())],
        )?
        .extract()?;
        let action_name = match status.as_str() {
            "pending_archive" => "ARCHIVE_AND_VIEW",
            "pending_view" => "VIEW_ONLY",
            "live" | "expired" => "LIVE",
            _ => return Ok(None),
        };
        if status == PENDING_ARCHIVE || status == PENDING_VIEW {
            let reason = self.skip(
                &location(&self.runtime, &native_field(history, "old")?)?,
                py.None().into_bound(py),
                &retention(item, &self.retention)?,
            )?;
            if !reason.is_none() {
                return model(
                    py,
                    RecordId::OldNameViewPlanEntry,
                    &[
                        (
                            "model_name",
                            native_field(&native_field(history, "move")?, "destination_model")?,
                        ),
                        (
                            "origin",
                            location(&self.runtime, &native_field(history, "old")?)?,
                        ),
                        (
                            "destination",
                            location(&self.runtime, &native_field(history, "new")?)?,
                        ),
                        ("action", action(&self.runtime, "NONE")?),
                        (
                            "target_name",
                            native_field(&native_field(history, "move")?, "target_name")?,
                        ),
                        (
                            "migration_event_id",
                            native_field(&native_field(history, "move")?, "event_id")?,
                        ),
                        ("reason", reason),
                    ],
                )
                .map(Some);
            }
        }
        let retention = native_field(&native_field(history, "required")?, "view_retention")?;
        let duration = record_class(py, RecordId::Duration)?.call_method1(
            "parse",
            (if retention.is_truthy()? {
                retention.clone()
            } else {
                text(py, "")
            },),
        )?;
        let created = native_field(history, "created")?;
        let expires_at = if !created.is_none() {
            native_field(&created, "expires_at")?
        } else if duration.is_none() {
            py.None().into_bound(py)
        } else {
            duration.call_method1("add_to", (&self.now,))?
        };
        let grants = if created.is_none() {
            py.None().into_bound(py)
        } else {
            native_field(&created, "grants_copied")?
        };
        model(
            py,
            RecordId::OldNameViewPlanEntry,
            &[
                ("model_name", name_object(item)?),
                (
                    "origin",
                    location(&self.runtime, &native_field(history, "old")?)?,
                ),
                (
                    "destination",
                    location(&self.runtime, &native_field(history, "new")?)?,
                ),
                ("action", action(&self.runtime, action_name)?),
                (
                    "target_name",
                    native_field(&native_field(history, "move")?, "target_name")?,
                ),
                ("retention", retention),
                ("expires_at", expires_at),
                (
                    "column_aliases",
                    if created.is_none() {
                        self.aliases(item)?
                    } else {
                        native_field(&created, "column_aliases")?
                    },
                ),
                (
                    "stores_history",
                    PyBool::new(py, stores_history(item)?).to_owned().into_any(),
                ),
                (
                    "migration_event_id",
                    native_field(&native_field(history, "move")?, "event_id")?,
                ),
                (
                    "grants_copied",
                    if grants.is_none() {
                        grants
                    } else {
                        imported(py, "builtins", "len")?.call1((grants,))?
                    },
                ),
                (
                    "grants_supported",
                    runtime_dialect(&self.runtime)?.syntax("relation_grants_supported")?,
                ),
                (
                    "archived",
                    PyBool::new(py, !native_field(history, "archived")?.is_none())
                        .to_owned()
                        .into_any(),
                ),
            ],
        )
        .map(Some)
    }
}

fn action<'py>(runtime: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    enum_member(runtime.py(), RecordId::OldNameViewAction, name)
}
