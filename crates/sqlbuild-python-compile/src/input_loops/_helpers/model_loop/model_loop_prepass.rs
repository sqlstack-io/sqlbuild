//! The model loop's header facts, computed on the compile pool off the GIL.

use std::sync::Arc;

use sqlbuild_discovery::model_files::main::header_facts::header_facts;
use sqlbuild_discovery::model_files::models::HeaderFacts;
use sqlbuild_python_base::native_threads::main::pooled::pooled;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::input_loops::_helpers::model::model_loop_host::ModelLoopRow;

/// A model's native header values and the relative path parse messages name.
pub(crate) type NativeHeader = (Arc<[(String, AuthoredValue)]>, String);

/// Each model's header facts on the compile pool, or in turn when the pool cannot start.
pub(crate) fn all_header_facts(headers: &[Option<NativeHeader>]) -> Vec<Option<HeaderFacts>> {
    pooled(headers, facts_of)
}

fn facts_of(header: &Option<NativeHeader>) -> Option<HeaderFacts> {
    let (values, path) = header.as_ref()?;
    Some(header_facts(values, path))
}

/// The row's native header values and relative path, unless Python constructed its file.
pub(crate) fn native_header(row: &ModelLoopRow) -> Option<NativeHeader> {
    let values: &Arc<[(String, AuthoredValue)]> = row.model_file.get().native_header_values()?;
    Some((Arc::clone(values), row.paths.relative_path.clone()))
}
