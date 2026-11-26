//! A `@loader` as the Python source SQLLoad's engine runs: its declaration as JSON, which the host completes at run time.

use crate::loads::_helpers::loaders::{python_source, table_settings};
use crate::loads::models::{LoadProblem, LoaderDeclaration};

/// `loader` as the JSON of SQLLoad's `PythonSource` (`name`, `module`, `settings`), its table under the loader's name.
pub fn loader_source(loader: &LoaderDeclaration) -> Result<serde_json::Value, LoadProblem> {
    let source = python_source(loader, &table_settings(loader))?;
    Ok(serde_json::json!({
        "name": source.name,
        "module": source.module,
        "settings": source.settings,
    }))
}
