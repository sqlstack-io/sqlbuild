//! `sqb server migrate`: apply the control store's pending schema migrations, or list them with `--dry-run`.

use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::main::plan_store_migration::plan_store_migration;
use sqlbuild_orchestration::store::models::StoreMigrationPlan;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::decision::{
    control_database, control_database_variable, flag,
};
use crate::native_commands::_helpers::terminal::{finish_lost_stdout, write_output};
use crate::server_config::main::read_server_config::read_server_config;

/// Run `sqb server migrate` on `runtime` and return its exit code.
pub(crate) fn run_store_migration(namespace: &Namespace, runtime: &tokio::runtime::Runtime) -> i32 {
    drop(write_output(&[(
        Stream::Stderr,
        "Opening control store...\n",
    )]));
    match runtime.block_on(store_migration(
        namespace,
        flag(namespace, "control_dry_run"),
    )) {
        Ok(report) => {
            let lost = write_output(&[(Stream::Stdout, &report)]);
            let exit_code = finish_lost_stdout(lost.as_ref(), 0);
            if exit_code == 0 {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Control store migration completed successfully\n",
                )]));
            }
            exit_code
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Control store migration failed: {error}\n"),
            )]));
            1
        }
    }
}

async fn store_migration(namespace: &Namespace, dry_run: bool) -> Result<String, ControlError> {
    let config = read_server_config(namespace)?;
    let url = control_database(
        namespace,
        &control_database_variable(namespace, &config.server),
    )?;
    let store = open_store(&url).await?;
    let plan = plan_store_migration(&store).await?;
    if !dry_run {
        migrate_store(&store).await?;
    }
    Ok(describe(&plan, dry_run))
}

/// The plan as lines for a person: what would happen with `--dry-run`, what happened without it.
fn describe(plan: &StoreMigrationPlan, dry_run: bool) -> String {
    let (create, upgrade, record) = if dry_run {
        ("would create", "would migrate", "would record")
    } else {
        ("created", "migrated", "recorded")
    };
    match plan {
        StoreMigrationPlan::Create { version } => {
            format!("The control store is empty: {create} schema version {version}.\n")
        }
        StoreMigrationPlan::Current { version } => {
            format!("The control store is at schema version {version}: nothing to migrate.\n")
        }
        StoreMigrationPlan::Upgrade { from, to, names } if names.is_empty() => format!(
            "The control store's schema is version {from} without a recorded version: \
             {record} version {to}.\n"
        ),
        StoreMigrationPlan::Upgrade { from, to, names } => format!(
            "The control store {upgrade} from schema version {from} to {to}:\n{}",
            names
                .iter()
                .map(|name| format!("  {name}\n"))
                .collect::<String>()
        ),
    }
}
