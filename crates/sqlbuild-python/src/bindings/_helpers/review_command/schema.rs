//! The run-owned review schema: its name, its target, and its creation and cleanup.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::janitor_planning::models::ArchiveTimestamp;
use sqlbuild_analysis::review_command::constants::UNKNOWN_REVIEW_DATABASE_CODE;
use sqlbuild_analysis::review_command::main::review_schema_name::review_schema_name;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::command_runtime::connection::resolve_target_connection_config;
use crate::bindings::_helpers::janitor::janitor_keys::utc_datetime;
use crate::bindings::_helpers::review_command::planning::{ReviewProject, ReviewSettings};
use crate::bindings::_helpers::review_command::support::review_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::none::none;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;

/// Where the review builds and where its ownership marker goes.
pub(crate) struct ReviewSchema {
    pub(crate) name: String,
    /// The database every node the review builds goes to: the active target's expanded one.
    pub(crate) database: Option<String>,
    marker_schema: String,
    run_id: String,
    created_at: ArchiveTimestamp,
}

/// This run's review schema, refusing nodes that set a database when the target sets none.
pub(crate) fn review_schema(
    py: Python<'_>,
    (project, settings, validated): (&ReviewProject, &ReviewSettings, &[String]),
    ((created_at, compact), head, run_id): ((ArchiveTimestamp, String), &str, &str),
) -> PyResult<ReviewSchema> {
    let Some(marker_schema) = project.target_schema.clone() else {
        return Err(review_error(
            py,
            "C771",
            &format!(
                "target '{}' has no schema to record the review schema in",
                settings.target
            ),
            Some("set schema on the target in sqlbuild_project.toml"),
        ));
    };
    if project.target_database.is_none() {
        let own: Vec<String> = validated
            .iter()
            .filter_map(|name| own_database(project, name))
            .collect();
        if !own.is_empty() {
            return Err(review_error(
                py,
                UNKNOWN_REVIEW_DATABASE_CODE,
                &format!(
                    "target '{}' sets no database, so the review cannot keep these nodes in one \
                     review database: {}",
                    settings.target,
                    own.join(", ")
                ),
                Some("set database on the target in sqlbuild_project.toml"),
            ));
        }
    }
    Ok(ReviewSchema {
        name: review_schema_name(&settings.schema_prefix, &compact, head, run_id),
        database: project.target_database.clone(),
        marker_schema,
        run_id: run_id.to_owned(),
        created_at,
    })
}

/// `name (database)` when the node sets its own database.
fn own_database(project: &ReviewProject, name: &str) -> Option<String> {
    let database: &String = project.databases.get(name)?;
    Some(format!("{name} ({database})"))
}

fn active_target_config<'py>(
    discovered: &Bound<'py, PyAny>,
    target: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &text(py, target),
    )
}

/// `discovered` plus the active target writing only to the review schema and database.
pub(crate) fn with_review_target<'py>(
    discovered: &Bound<'py, PyAny>,
    settings: &ReviewSettings,
    schema: &ReviewSchema,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let active: Bound<'py, PyAny> = active_target_config(discovered, &settings.target)?;
    let against: Bound<'py, PyAny> = active_target_config(discovered, &settings.against)?;
    let sources_from: Bound<'py, PyAny> = match native_field(&against, "defer_sources_to")? {
        value if !value.is_none() => value,
        _ => text(py, &settings.against),
    };
    let clone_policy: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::ClonePolicy,
        &[
            ("allow_as_clone_origin", boolean(py, false)),
            ("allow_as_clone_destination", boolean(py, true)),
        ],
    )?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("schema", text(py, &schema.name)),
        ("loader_schema", text(py, &schema.name)),
        ("defer_sources_to", sources_from),
        ("defer_clone_from", none(py)),
        ("clone", clone_policy),
        (
            "missing_migration_origin",
            enum_member(py, RecordId::MissingMigrationOriginPolicy, "ALLOW")?,
        ),
    ];
    if let Some(database) = &schema.database {
        fields.push(("database", text(py, database)));
    }
    let review: Bound<'py, PyAny> = replaced(&active, &fields)?;
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let targets: Bound<'py, PyDict> = PyDict::new(py);
    targets.update(native_field(&project_config, "targets")?.cast::<pyo3::types::PyMapping>()?)?;
    targets.set_item(&schema.name, review)?;
    let project_config: Bound<'py, PyAny> =
        replaced(&project_config, &[("targets", targets.into_any())])?;
    replaced(discovered, &[("project_config", project_config)])
}

/// The active target's connection config.
pub(crate) fn connection_config<'py>(
    request: &Bound<'py, PyAny>,
    (project_dir, discovered): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    settings: &ReviewSettings,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    resolve_target_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", project_dir.clone()),
                ("target_name", text(py, &settings.target)),
                ("cli_vars", native_field(request, "cli_vars")?),
            ],
        )?),
    )
}

/// Record ownership in the active target's state table, then create the review schema.
pub(crate) fn create_review_schema(
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    schema: &ReviewSchema,
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    call_with(
        &host_entry(py, ExecutorHost::CreateReviewSchema)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
            ("database", optional(py, schema.database.as_deref())),
            ("schema", text(py, &schema.name)),
            ("marker_database", optional(py, schema.database.as_deref())),
            ("marker_schema", text(py, &schema.marker_schema)),
            ("run_id", text(py, &schema.run_id)),
            ("created_at", utc_datetime(py, schema.created_at)?),
        ],
    )?;
    Ok(())
}

/// Drop the review schema; whether it is gone afterwards.
pub(crate) fn drop_review_schema(
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    schema: &ReviewSchema,
) -> PyResult<bool> {
    let py: Python<'_> = adapter.py();
    call_with(
        &host_entry(py, ExecutorHost::DropReviewSchema)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
            ("database", optional(py, schema.database.as_deref())),
            ("schema", text(py, &schema.name)),
        ],
    )?
    .is_truthy()
}

fn optional<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    value.map_or_else(|| none(py), |value| text(py, value))
}
