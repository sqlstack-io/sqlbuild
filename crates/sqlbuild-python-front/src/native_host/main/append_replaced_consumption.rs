//! Leave relations replaced outside a recorded build without a completed consumption record.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_state::consumption::main::replaced_append::replaced_append;
use sqlbuild_state::consumption::models::{ConsumptionAppend, NodeKey, RelationKey};
use uuid::Uuid;

use crate::native_host::_helpers::consumption_io::{append, table_exists};
use crate::native_host::models::Location;

/// Append a never-completed execution for each of `replaced` where its schema has a consumption table.
pub fn append_replaced_consumption(
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    run_id: &str,
    replaced: Vec<(NodeKey, RelationKey)>,
) -> PyResult<()> {
    let mut rows: BTreeMap<Location, Vec<ConsumptionAppend>> = BTreeMap::new();
    for (node, relation) in replaced {
        let execution_id: String = Uuid::now_v7().to_string();
        rows.entry(Location::of(&relation))
            .or_default()
            .push(replaced_append((node, relation), (&execution_id, run_id)));
    }
    for (location, rows) in rows {
        if table_exists(adapter, connection, &location)? {
            append(adapter, connection, &location, &rows)?;
        }
    }
    Ok(())
}
