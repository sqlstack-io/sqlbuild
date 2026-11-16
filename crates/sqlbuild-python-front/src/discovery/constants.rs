//! Public constants of discovery.

use crate::discovery::_helpers::records::getters::read_native_directory_snapshot;
use crate::discovery::_helpers::records::getters::read_native_model_file;
use crate::discovery::_helpers::snapshots::native_classes::NativeDirectorySnapshot;
use crate::discovery::models::NativeModelFile;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The fields a SQL hook entry reports, in output order.
pub const SQL_HOOK_OUTPUT_FIELDS: [&str; 6] = [
    "statement",
    "name",
    "relative_path",
    "definition_sql",
    "kwargs",
    "description",
];

/// The flag of the static commands that lets them compile without provider secrets.
pub(crate) const ALLOW_MISSING_SECRETS_FLAG: &str = "--allow-missing-secrets";

/// The environment variable that sets `--allow-missing-secrets`.
pub const ALLOW_MISSING_SECRETS_ENV: &str = "SQLBUILD_ALLOW_MISSING_SECRETS";

/// The `[settings]` key of `sqlbuild_local.toml` that sets `--allow-missing-secrets`.
pub const ALLOW_MISSING_SECRETS_SETTING: &str = "allow_missing_secrets";

/// Environment values that switch a boolean `SQLBUILD_*` variable on.
pub const TRUE_ENV_VALUES: [&str; 4] = ["1", "on", "true", "yes"];

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<NativeModelFile>,
        read: read_native_model_file,
    },
    GetterEntry {
        class: class_of::<NativeDirectorySnapshot>,
        read: read_native_directory_snapshot,
    },
];
