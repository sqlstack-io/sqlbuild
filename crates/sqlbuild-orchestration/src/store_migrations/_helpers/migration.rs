//! One schema template, rendered only at the database type boundary, and the numbered migrations to it.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction};
use crate::store_migrations::_helpers::old_form_sensors::drop_old_form_sensors;
use crate::store_migrations::_helpers::recall_schema::recall_schema;
use crate::store_migrations::constants::{
    BASELINE_VERSION, POSTGRES_BASELINE_CHECKSUMS, SQLITE_BASELINE_CHECKSUMS,
};
use crate::store_migrations::models::{StoreMigration, StoreMigrationPlan};

use crate::store::constants::{INDEXES, SCHEMA};

/// The migrations after the baseline, in order: entry `i` includes `store_migrations/sql/{NNNN}_{name}.{sqlite,postgres}.sql` and brings a store to version `NNNN = BASELINE_VERSION + i + 1`.
pub(crate) const MIGRATIONS: &[StoreMigration] = &[
    StoreMigration {
        name: "large_project_server",
        sqlite: include_str!("../sql/0002_large_project_server.sqlite.sql"),
        postgres: include_str!("../sql/0002_large_project_server.postgres.sql"),
        step: None,
    },
    StoreMigration {
        name: "attention_acknowledgement_index",
        sqlite: include_str!("../sql/0003_attention_acknowledgement_index.sqlite.sql"),
        postgres: include_str!("../sql/0003_attention_acknowledgement_index.postgres.sql"),
        step: None,
    },
    StoreMigration {
        name: "drop_concurrency_source",
        sqlite: include_str!("../sql/0004_drop_concurrency_source.sqlite.sql"),
        postgres: include_str!("../sql/0004_drop_concurrency_source.postgres.sql"),
        step: None,
    },
    StoreMigration {
        name: "sensor_functions",
        sqlite: include_str!("../sql/0005_sensor_functions.sqlite.sql"),
        postgres: include_str!("../sql/0005_sensor_functions.postgres.sql"),
        step: Some(drop_old_form_sensors),
    },
    StoreMigration {
        name: "node_results",
        sqlite: include_str!("../sql/0006_node_results.sqlite.sql"),
        postgres: include_str!("../sql/0006_node_results.postgres.sql"),
        step: None,
    },
    StoreMigration {
        name: "run_output",
        sqlite: include_str!("../sql/0007_run_output.sqlite.sql"),
        postgres: include_str!("../sql/0007_run_output.postgres.sql"),
        step: None,
    },
    StoreMigration {
        name: "discovered_document",
        sqlite: include_str!("../sql/0008_discovered_document.sqlite.sql"),
        postgres: include_str!("../sql/0008_discovered_document.postgres.sql"),
        step: None,
    },
];

/// The schema version a store created by this build has.
pub(crate) fn current_version(migrations: &[StoreMigration]) -> i64 {
    BASELINE_VERSION + i64::try_from(migrations.len()).unwrap_or(i64::MAX - BASELINE_VERSION)
}

pub(crate) fn statements(sql: &str) -> impl Iterator<Item = &str> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
}

/// The checksum of the schema this build creates, recorded with its version.
pub(crate) fn schema_digest(store: &ControlStore) -> String {
    schema_text_digest(&recall_schema(store).join(";\n"), SCHEMA, INDEXES)
}

/// The checksum of a schema's Recall DDL, `schema.sql` and `indexes.sql`, whatever line endings Git used.
pub(crate) fn schema_text_digest(recall: &str, schema: &str, indexes: &str) -> String {
    sha256_hex(
        format!("{recall}\n{schema}\n{indexes}")
            .replace("\r\n", "\n")
            .as_bytes(),
    )
}

/// The statements of `migration` for `store`'s database type.
pub(crate) fn migration_sql(store: &ControlStore, migration: &StoreMigration) -> String {
    render(
        store,
        match store {
            ControlStore::Sqlite(_) => migration.sqlite,
            ControlStore::Postgres(_) => migration.postgres,
        },
    )
}

/// The store's recorded `(checksum, version)`; the version is `None` for stores from before versioning.
pub(crate) async fn recorded_schema(
    store: &ControlStore,
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Option<(String, Option<i64>)>), ControlError> {
    let columns = match store {
        ControlStore::Sqlite(_) => "SELECT name FROM pragma_table_info('sqb_schema_version')",
        ControlStore::Postgres(_) => {
            "SELECT column_name AS name FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = 'sqb_schema_version'"
        }
    };
    let (transaction, columns) = query(transaction, columns, vec![]).await?;
    let has = |name: &str| columns.iter().any(|row| row["name"].as_str() == Some(name));
    if !has("checksum") {
        return Ok((transaction, None));
    }
    let select = if has("version") {
        "SELECT checksum, version FROM sqb_schema_version"
    } else {
        "SELECT checksum, NULL AS version FROM sqb_schema_version"
    };
    let (transaction, rows) = query(transaction, select, vec![]).await?;
    let recorded = rows.first().map(|row| {
        (
            row["checksum"].as_str().unwrap_or_default().to_owned(),
            row["version"].as_i64(),
        )
    });
    Ok((transaction, recorded))
}

/// What opening a store with `recorded` schema does, or why this build refuses it.
pub(crate) fn migration_plan(
    store: &ControlStore,
    recorded: Option<(&str, Option<i64>)>,
    migrations: &[StoreMigration],
) -> Result<StoreMigrationPlan, ControlError> {
    let current = current_version(migrations);
    let Some((checksum, version)) = recorded else {
        return Ok(StoreMigrationPlan::Create { version: current });
    };
    let baseline_checksums = match store {
        ControlStore::Sqlite(_) => SQLITE_BASELINE_CHECKSUMS,
        ControlStore::Postgres(_) => POSTGRES_BASELINE_CHECKSUMS,
    };
    let version = match version {
        Some(version) => version,
        None if baseline_checksums.contains(&checksum) => BASELINE_VERSION,
        None => return Err(refusal(store, Refusal::OlderThanBaseline)),
    };
    if version > current {
        return Err(refusal(store, Refusal::Newer { version, current }));
    }
    if version < BASELINE_VERSION {
        return Err(refusal(store, Refusal::OlderThanBaseline));
    }
    if version == current && !matches!(recorded, Some((_, None))) {
        return if checksum == schema_digest(store) {
            Ok(StoreMigrationPlan::Current { version })
        } else {
            Err(refusal(store, Refusal::Mismatch { version }))
        };
    }
    let pending = &migrations[usize::try_from(version - BASELINE_VERSION).unwrap_or(0)..];
    Ok(StoreMigrationPlan::Upgrade {
        from: version,
        to: current,
        names: pending
            .iter()
            .zip(version + 1..)
            .map(|(migration, number)| format!("{number:04}_{}", migration.name))
            .collect(),
    })
}

enum Refusal {
    Newer { version: i64, current: i64 },
    OlderThanBaseline,
    Mismatch { version: i64 },
}

/// Why this build refuses a store, and how to start a new one.
fn refusal(store: &ControlStore, refusal: Refusal) -> ControlError {
    let (scope, command) = recreate_command(store);
    let recreate = format!(
        "To start a new, empty store, which discards its run history{scope}, stop every \
         server using it and run: {command}"
    );
    ControlError::Schema(match refusal {
        Refusal::Newer { version, current } => format!(
            "this control store's schema is version {version}, newer than this build's \
             version {current}; upgrade this server to the release that last opened it or a later one"
        ),
        Refusal::OlderThanBaseline => format!(
            "this control store's schema is older than version {BASELINE_VERSION}, the oldest \
             this build upgrades; open it once with an earlier release that still has the \
             migrations it needs, then with this one. {recreate}"
        ),
        Refusal::Mismatch { version } => format!(
            "this control store's schema doesn't match this build's, although both are version \
             {version}; the build's schema changed without a migration. {recreate}"
        ),
    })
}

/// The scope of what recreating `store` discards, and the command that does it.
fn recreate_command(store: &ControlStore) -> (&'static str, String) {
    let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
    match store {
        ControlStore::Sqlite(path) => {
            let path = path.to_string_lossy();
            let command = format!(
                "rm -f {} {} {}",
                quote(&path),
                quote(&format!("{path}-wal")),
                quote(&format!("{path}-shm"))
            );
            ("", command)
        }
        ControlStore::Postgres(pool) => {
            let options = pool.connect_options();
            let host = options.get_socket().map_or_else(
                || options.get_host().to_owned(),
                |socket| socket.to_string_lossy().into_owned(),
            );
            let target = format!(
                "-h {} -p {} -U {} {}",
                quote(&host),
                options.get_port(),
                quote(options.get_username()),
                quote(options.get_database().unwrap_or(options.get_username()))
            );
            (
                " and everything else in that database",
                format!("dropdb {target} && createdb {target}"),
            )
        }
    }
}

pub(crate) fn render(store: &ControlStore, sql: &str) -> String {
    let postgres = matches!(store, ControlStore::Postgres(_));
    sql.replace("{uuid}", if postgres { "UUID" } else { "TEXT" })
        .replace("{time}", if postgres { "TIMESTAMPTZ" } else { "TEXT" })
        .replace("{binary}", if postgres { "BYTEA" } else { "BLOB" })
        .replace("{json}", if postgres { "JSONB" } else { "JSON" })
}
