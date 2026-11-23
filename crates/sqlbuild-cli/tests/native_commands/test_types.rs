use std::path::PathBuf;
use std::process::Child;

pub(crate) struct CommandTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_success: bool,
    pub(crate) expected_stdout_prefix: &'static str,
    pub(crate) expected_stderr: &'static str,
}

pub(crate) struct TriggerCursorTestCase {
    pub(crate) description: &'static str,
    pub(crate) change: &'static str,
    pub(crate) expected_success: bool,
}

pub(crate) struct OpenApiTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_path: &'static str,
}

pub(crate) struct ServiceTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_leaders: i64,
    pub(crate) expected_heartbeats: i64,
}

pub(crate) struct PasswordCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) password: &'static str,
    pub(crate) replacement: &'static str,
    pub(crate) expected_password_rows: i64,
    pub(crate) expected_audits: i64,
}

pub(crate) struct PasswordOriginTestCase {
    pub(crate) description: &'static str,
    pub(crate) origin: &'static str,
    pub(crate) expected_error: &'static str,
}

pub(crate) struct AdministratorCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_administrators: i64,
}

pub(crate) struct ProxyCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) subject: &'static str,
    pub(crate) expected_statuses: [reqwest::StatusCode; 3],
}

pub(crate) struct ProxyStartupTestCase {
    pub(crate) description: &'static str,
    pub(crate) configuration: &'static str,
    /// The username the refused requests carry, which the log must never show.
    pub(crate) username: &'static str,
    pub(crate) host: &'static str,
    pub(crate) expected_status: reqwest::StatusCode,
    pub(crate) expected_warnings: &'static [(&'static str, usize)],
}

/// A server with the default `trusted_peers`, reached over a non-loopback address of this machine.
pub(crate) struct ProxyRemotePeerTestCase {
    pub(crate) description: &'static str,
    pub(crate) configuration: &'static str,
    /// The username the remote requests carry, which the log must never show.
    pub(crate) username: &'static str,
    pub(crate) expected_status: reqwest::StatusCode,
    pub(crate) expected_warnings: usize,
}

/// A trusted-proxy server over plain HTTP, open to the network and reached by a peer that isn't the proxy.
#[cfg(unix)]
pub(crate) struct NetworkKeyTestCase {
    pub(crate) description: &'static str,
    pub(crate) configuration: &'static str,
    /// The Host the peer sends, neither the origin's nor an allowed one.
    pub(crate) host: &'static str,
    /// A keyed read and write, then the username header without a key under that Host and the origin's.
    pub(crate) expected_statuses: [reqwest::StatusCode; 4],
    /// Whom the keyed read authenticated as.
    pub(crate) expected_name: &'static str,
}

pub(crate) struct ServerConfigurationTestCase {
    pub(crate) description: &'static str,
    pub(crate) configuration: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_error: &'static str,
}

pub(crate) struct TestControl {
    pub(crate) child: Child,
    pub(crate) directory: tempfile::TempDir,
    pub(crate) stderr: PathBuf,
}

impl Drop for TestControl {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) struct LocationHealthTestCase {
    pub(crate) description: &'static str,
    /// Health reports the location sends before the check, oldest first, as JSON `health` values.
    pub(crate) reports: &'static [&'static str],
    /// Connection reports another target of the same location sends afterwards.
    pub(crate) other_target_connections: usize,
    pub(crate) expected_state: &'static str,
    pub(crate) expected_exit: i32,
}

pub(crate) struct StoreMigrationCommandTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_stdout: &'static str,
    /// Whether the SQLite store file exists after the command.
    pub(crate) expected_store: bool,
}

pub(crate) struct GeneratedKeyCliTestCase {
    pub(crate) description: &'static str,
    /// The arguments of `sqb admin create-service-account` after the name.
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_role: &'static str,
    /// What the key's bootstrap read answers: when made, in the overlap, once replaced, disabled, enabled.
    pub(crate) expected_statuses: [u16; 5],
}

pub(crate) struct ImportedKeyCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) key: &'static str,
    pub(crate) expected_status: u16,
    /// Whether the JSON result repeats the key.
    pub(crate) expected_key_printed: bool,
}

pub(crate) struct RefusedServiceAccountCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    /// The value of `SQB_TEST_IMPORTED_KEY`.
    pub(crate) imported_key: &'static str,
    pub(crate) expected_error: &'static str,
}

/// An admin command without `--json`, run after one location, person, token and service account exist.
pub(crate) struct AdminOutputTestCase {
    pub(crate) description: &'static str,
    /// Arguments, with `{location}`, `{person}` and `{token}` filled in.
    pub(crate) arguments: &'static [&'static str],
    /// How stdout starts, and how many lines it has.
    pub(crate) expected_stdout: (&'static str, usize),
    /// How the one stderr line starts; empty for none.
    pub(crate) expected_line: &'static str,
}

/// A store that opens verbosely or slowly.
#[cfg(unix)]
pub(crate) struct AdminOpeningTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    /// How long another writer holds the new store.
    pub(crate) held_milliseconds: u64,
    pub(crate) expected_stdout_empty: bool,
    pub(crate) expected_stderr_prefix: &'static str,
}

/// A committed secret that stdout couldn't take.
#[cfg(unix)]
pub(crate) struct SecretDeliveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) expected_exit: i32,
    pub(crate) expected_stderr: &'static str,
}

/// One invocation against an `http://` network origin, and what the client did with the credential.
pub(crate) struct RepublishCliTestCase {
    pub(crate) description: &'static str,
    pub(crate) key: Option<&'static str>,
    pub(crate) states: &'static [&'static str],
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_exit: i32,
    pub(crate) expected_methods: &'static [&'static str],
    pub(crate) expected_warnings: usize,
    pub(crate) expected_stderr: &'static str,
}
