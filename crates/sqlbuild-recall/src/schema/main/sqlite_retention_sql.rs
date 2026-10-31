//! `sqlite_retention_sql`: the deletes SQLite maintenance runs, all rows of an invocation together.

use sqlbuild_core::identifier_text::main::quote_identifier::quote_identifier as sqlite_identifier;

use crate::schema::constants::{
    INVOCATION_TABLES, META_MAINTENANCE_COMPONENT, RETENTION_CANDIDATE_TABLES,
};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::SqliteRetentionSql;

fn expired_invocations() -> String {
    let candidates: Vec<String> = RETENTION_CANDIDATE_TABLES
        .iter()
        .map(|table| {
            format!(
                "SELECT invocation_id FROM {} WHERE invocation_started_at < ?1",
                sqlite_identifier(table_spec(*table).name)
            )
        })
        .collect();
    format!(
        "SELECT DISTINCT invocation_id FROM (\n    {}\n)\n\
         WHERE invocation_id NOT IN (\n\
         \x20   SELECT beat.invocation_id FROM recall_heartbeats AS beat\n\
         \x20   WHERE (beat.stale_at >= ?2 OR beat.beat_at >= ?1) AND NOT EXISTS (\n\
         \x20       SELECT 1 FROM recall_events AS terminal\n\
         \x20       WHERE terminal.invocation_id = beat.invocation_id\n\
         \x20       AND terminal.event_type IN ('invocation_completed', 'invocation_failed')\n\
         \x20   )\n\
         )\nLIMIT ?3",
        candidates.join("\n    UNION ALL ")
    )
}

/// The maintenance statements; running invocations, and unfinished ones that beat within retention, such as a suspended run, are never deleted.
#[must_use]
pub fn sqlite_retention_sql() -> SqliteRetentionSql {
    SqliteRetentionSql {
        expired_invocations: expired_invocations(),
        delete_invocations: INVOCATION_TABLES
            .iter()
            .map(|table| {
                (
                    *table,
                    format!(
                        "DELETE FROM {} WHERE invocation_id IN (SELECT value FROM json_each(?1))",
                        sqlite_identifier(table_spec(*table).name)
                    ),
                )
            })
            .collect(),
        delete_orphan_artifacts: "DELETE FROM recall_artifacts WHERE seen_at < ?1 AND NOT EXISTS (\
             SELECT 1 FROM recall_artifact_refs AS ref WHERE ref.digest = recall_artifacts.digest)"
            .to_owned(),
        delete_old_claims: format!(
            "DELETE FROM recall_meta WHERE component = '{META_MAINTENANCE_COMPONENT}' \
             AND recorded_at < ?1"
        ),
    }
}
