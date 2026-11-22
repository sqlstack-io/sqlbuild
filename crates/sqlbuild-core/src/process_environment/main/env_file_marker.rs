//! Which env file this process tree loaded.

use serde_json::Value;

use crate::process_environment::constants::ENV_FILE_MARKER_VARIABLE;
use crate::process_environment::main::environment_var_os::environment_var_os;
use crate::process_environment::models::EnvFileMarker;

fn names(value: &Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)?
        .as_array()?
        .iter()
        .map(|name| name.as_str().map(str::to_owned))
        .collect()
}

fn optional_text(value: &Value, key: &str) -> Option<Option<String>> {
    match value.get(key)? {
        Value::Null => Some(None),
        Value::String(text) => Some(Some(text.clone())),
        _ => None,
    }
}

/// The marker in the effective environment, `None` when nothing loaded a file or it is malformed.
#[must_use]
pub fn env_file_marker() -> Option<EnvFileMarker> {
    let text: String = match environment_var_os(ENV_FILE_MARKER_VARIABLE)?.into_string() {
        Ok(text) => text,
        Err(_not_unicode) => return None,
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_malformed) => return None,
    };
    Some(EnvFileMarker {
        project_dir: value.get("project_dir")?.as_str()?.to_owned(),
        target: optional_text(&value, "target")?,
        path: optional_text(&value, "path")?,
        loaded: value.get("loaded")?.as_bool()?,
        names: names(&value, "names")?,
        overridden: names(&value, "overridden")?,
    })
}
