//! The ownership header and freshness of the generated rules skill.

use pyo3::prelude::{PyResult, Python};

use crate::bindings::_helpers::rules::selection::rules_error;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

const SKILL_OWNER: &str = "sqlbuild";
const SKILL_IDENTITY: &str = "sqlbuild-rules";

/// Generated guidance with its ownership header; a policy failure raises `RulesError`.
pub(crate) fn owned_skill(
    py: Python<'_>,
    content: &str,
    input_fingerprint: &str,
) -> PyResult<String> {
    let rendered: Result<String, String> = compiler_guard(|| {
        Ok(fensu_policy::render_owned_skill(
            SKILL_OWNER,
            SKILL_IDENTITY,
            input_fingerprint,
            content.as_bytes(),
        )
        .map_err(|error| error.to_string())
        .and_then(|value| String::from_utf8(value).map_err(|error| error.to_string())))
    })?;
    rendered.map_err(|message| rules_error(py, &message, None))
}

/// Whether generated guidance is current, stale, missing or foreign under the ownership contract.
pub(crate) fn skill_state(content: Option<&str>, input_fingerprint: &str) -> String {
    let freshness = fensu_policy::skill_freshness(
        content.map(str::as_bytes),
        SKILL_OWNER,
        SKILL_IDENTITY,
        input_fingerprint,
    );
    format!("{freshness:?}").to_lowercase()
}
