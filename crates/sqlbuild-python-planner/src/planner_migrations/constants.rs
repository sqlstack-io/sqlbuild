//! Public constants of planner migrations.

use crate::planner_migrations::_helpers::projections::getters::read_migration_state_inspection;
use crate::planner_migrations::models::MigrationStateInspection;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

pub(crate) const AUTOMATIC: &str = "automatic";

pub(crate) const DONE: &str = "done";

pub(crate) const PENDING_ARCHIVE: &str = "pending_archive";

pub(crate) const PENDING_VIEW: &str = "pending_view";

pub(crate) const LIST_RELATIONS_FOR_INSPECTION: AdapterHost =
    AdapterHost::ListRelationsForInspection;

pub(crate) const GET_COLUMNS_FOR_INSPECTION: AdapterHost = AdapterHost::GetColumnsForInspection;

pub(crate) const QUALIFIED_RELATION_MAX_PARTS: usize = 3;

pub(crate) const MIGRATION_MODEL_NAME_METADATA_KEY: &str = "model_name";

pub(crate) const HIDDEN_ORIGIN_REMEDY: &str = "the connection may lack permission to see it, or it was dropped outside SQLBuild; restore access to it, or remove migrate_from to build the model fresh deliberately (missing_migration_origin does not apply)";

pub(crate) const MISSING_ORIGIN_ALLOW_OUTCOME: &str = "nothing is migrated and the build continues as if migrate_from were absent; remove migrate_from once no target still needs it";

pub(crate) const MISSING_ORIGIN_CONFIRMATION_OUTCOME: &str = "building without it needs confirmation (missing_migration_origin = require_confirmation); pass --allow-missing-migration-origin, or remove migrate_from if it is no longer needed";

pub(crate) const MISSING_ORIGIN_DENY_OUTCOME: &str = "the build stops (missing_migration_origin = deny); if the migration already happened elsewhere or is no longer needed, remove migrate_from";

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<MigrationStateInspection>,
    read: read_migration_state_inspection,
}];
