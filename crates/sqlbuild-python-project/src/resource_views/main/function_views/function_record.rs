//! The function record of an attached function, before assembly resolves its dependencies.

use crate::resource_views::models::FunctionProjections;
use crate::resource_views::types::AttachedFunction;
use crate::resource_views::types::FunctionRecord;
use pyo3::sync::PyOnceLock;

/// The function record of an attached function, before assembly resolves its dependencies.
pub fn function_record(attached: AttachedFunction) -> FunctionRecord {
    FunctionRecord {
        attached,
        deps: None,
        host: FunctionProjections {
            arguments: PyOnceLock::new(),
            return_columns: PyOnceLock::new(),
            references: PyOnceLock::new(),
            key: PyOnceLock::new(),
        },
    }
}
