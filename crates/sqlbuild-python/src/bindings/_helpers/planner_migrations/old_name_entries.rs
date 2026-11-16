//! Assemble new and resumed old-name view steps from recorded facts.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::planner_migrations::column_objects::{model, text};
use crate::bindings::_helpers::planner_migrations::constants::{PENDING_ARCHIVE, PENDING_VIEW};
use crate::bindings::_helpers::planner_migrations::models::OldNamePlanner;
use crate::bindings::_helpers::planner_migrations::old_name_objects::{
    expires, location, relation, retention, stores_history,
};
use crate::bindings::_helpers::planner_migrations::old_names::declared_column_aliases;
use crate::bindings::_helpers::planner_migrations::old_names::old_name_view_skip_reason;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyBool, PyList, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

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
        })
    }

    fn newest(
        &self,
        destination: &Bound<'py, PyAny>,
        origin: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut newest: Option<(Bound<'py, PyTuple>, Bound<'py, PyAny>)> = None;
        for history in &self.histories {
            let movement = native_field(history, "move")?;
            if !native_field(&movement, "destination")?
                .call_method1("matches", (destination,))?
                .is_truthy()?
            {
                continue;
            }
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
        let Some(history) = self.newest(&relation(&model_reads::destination(item)?)?, None)? else {
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

    fn skip(
        &self,
        origin: &Bound<'py, PyAny>,
        tracked: Bound<'py, PyAny>,
        retention: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        object(
            self.runtime.py(),
            old_name_view_skip_reason(
                self.runtime.py(),
                Some(keywords(
                    self.runtime.py(),
                    &[
                        ("origin", origin.clone()),
                        ("origin_tracked", tracked),
                        ("retention", retention.clone()),
                        (
                            "models",
                            PyTuple::new(self.runtime.py(), &self.models)?.into_any(),
                        ),
                        ("views", self.views.clone()),
                    ],
                )?),
            )?,
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
                    self.runtime
                        .getattr("adapter")?
                        .getattr("relation_grants_supported")?,
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
                ("model_name", model_reads::name_object(item)?),
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
                    self.runtime
                        .getattr("adapter")?
                        .getattr("relation_grants_supported")?,
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
