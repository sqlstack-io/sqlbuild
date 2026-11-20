//! The foreground warehouse connection, the location child's pipes and the run feed of `sqb dev`.

use std::time::Duration;

/// The statuses of SQLBuild's own progress rows, which the framed block leaves out.
pub(crate) const LIFECYCLE_STATUSES: [&str; 3] = ["START", "OK", "FAIL"];
/// The `sqb debug` row that says whether the warehouse answered.
pub(crate) const CONNECTION_TEST_LABEL: &str = "connection test";
/// The warehouse state when `sqb debug` printed no report.
pub(crate) const NO_REPORT: &str = "sqb debug did not report a connection; see l for its log";
/// The `sqb debug` configuration row that names the adapter.
pub(crate) const ADAPTER_LABEL: &str = "adapter";
/// A `sqb debug` row that passed.
pub(crate) const OK_STATUS: &str = "OK";
/// A `sqb debug` row that failed.
pub(crate) const ERROR_STATUS: &str = "ERROR";
/// Report sections searched for the first error when the connection test did not pass.
pub(crate) const REPORT_ERROR_SECTIONS: [&str; 2] = ["configuration", "providers"];
/// How often finished and newly started runs are read for event lines.
pub(crate) const RUN_POLL: Duration = Duration::from_secs(2);
/// The newest runs read per poll.
pub(crate) const RUN_PAGE: u32 = 20;
/// The warehouse row's detail while the foreground connection runs.
pub(crate) const CONNECTING: &str = "connecting";
/// How long the foreground `sqb debug`'s pipes are drained after it exits.
pub(crate) const DEBUG_DRAIN: Duration = Duration::from_secs(2);
/// Connection settings that say where the warehouse is, in the order they are looked for.
pub(crate) const PLACE_KEYS: [&str; 3] = ["account", "host", "project"];
/// Connection settings that name the authentication method.
pub(crate) const AUTH_KEYS: [&str; 2] = ["authenticator", "method"];
/// Connection settings that mean key-pair authentication when no method is named.
pub(crate) const KEY_PAIR_KEYS: [&str; 2] = ["private_key_file", "private_key_path"];
/// Connection settings that name the role.
pub(crate) const ROLE_KEYS: [&str; 1] = ["role"];
/// Connection settings that name the compute or storage, a warehouse before a database.
pub(crate) const STORAGE_KEYS: [&str; 4] = ["warehouse", "database", "dataset", "path"];
/// Readable names for authenticators, keyed by their lower-case setting.
pub(crate) const AUTH_NAMES: [(&str, &str); 8] = [
    ("oauth_authorization_code", "browser sign-in (OAuth)"),
    ("externalbrowser", "browser sign-in (SSO)"),
    ("snowflake_jwt", "key pair"),
    ("snowflake", "password"),
    ("username_password_mfa", "password and MFA"),
    ("oauth", "OAuth token"),
    ("oauth_client_credentials", "OAuth client credentials"),
    ("programmatic_access_token", "access token"),
];
/// Setting values that say nothing on the Warehouse line.
pub(crate) const UNSHOWN_VALUES: [&str; 6] =
    ["none", "null", "true", "false", "not configured", "not set"];
/// Why `sqb dev` stops when the first warehouse connection fails.
pub(crate) const WAREHOUSE_FAILED: &str = "could not connect to the warehouse";
/// Why `sqb dev` stops when the code location exits by itself.
pub(crate) const LOCATION_STOPPED: &str = "the code location stopped unexpectedly";
/// The stop line of a warehouse connection that was still open.
pub(crate) const CANCELLED_CONNECT: &str = "Cancelled warehouse connection";
