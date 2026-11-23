use std::path::{Path, PathBuf};

use crate::bindings::_helpers::change_selection::base_cache::cache_key_with;

use crate::bindings::_helpers::cli::failure_cause::{FailureCause, report_failure_cause};

/// A connection failure's cause, as `sqb debug` reports it.
pub(crate) fn connection_cause() -> FailureCause {
    FailureCause {
        error_type: "ConnectionError".to_owned(),
        error_code: "C980".to_owned(),
        message: "connection test: refused".to_owned(),
    }
}

/// Report the cause on the calling thread, as the running invocation does.
pub(crate) fn report_here() {
    report_failure_cause(connection_cause());
}

/// Report the cause on another thread, as a concurrent invocation does.
pub(crate) fn report_elsewhere() {
    std::thread::spawn(report_here)
        .join()
        .expect("the reporting thread finishes");
}

/// A project whose model reads `ORDERS_REGION` through `@@ENV:`.
pub(super) fn project_reading_environment() -> PathBuf {
    let root: PathBuf = std::env::temp_dir()
        .join(format!("sqlbuild-since-cache-{}", std::process::id()))
        .join("project");
    let _ = std::fs::create_dir_all(root.join("sql/models"));
    let _ = std::fs::write(
        root.join("sql/models/orders.sql"),
        "SELECT order_id FROM orders WHERE region = '@@ENV:ORDERS_REGION'\n",
    );
    root
}

/// The base cache key of `project` with environment variables `values`.
pub(super) fn cache_key_under(project: &Path, values: &[(&str, &str)]) -> String {
    let lookup = |name: &str| -> Option<String> {
        values
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| (*value).to_owned())
    };
    cache_key_with(project, (None, "{}", ""), &lookup).expect("the project reads")
}
