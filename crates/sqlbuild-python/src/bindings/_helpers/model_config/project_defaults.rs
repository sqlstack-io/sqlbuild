//! The sparse mapping of a project's `[defaults]` that every model's config layers over.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each default in `project_defaults_to_mapping`'s order, and whether a falsy value is unset.
const DEFAULT_KEYS: [(&str, bool); 31] = [
    ("materialized", false),
    ("database", false),
    ("schema", false),
    ("contract", false),
    ("incremental_strategy", false),
    ("incremental_mode", false),
    ("microbatch_strategy", false),
    ("cursor_watermark_mode", false),
    ("merge_exclude_columns", true),
    ("full_refresh", false),
    ("append_cursor_inclusive", false),
    ("cursor_start", false),
    ("cursor_end", false),
    ("cursor_start_max_ahead", false),
    ("cursor_start_max_action", false),
    ("cursor_future_max_distance", false),
    ("cursor_future_action", false),
    ("lookback", false),
    ("batch_size", false),
    ("batch_concurrency", false),
    ("max_microbatches", false),
    ("max_duration", false),
    ("unaccounted_partition_policy", false),
    ("replay_on_change", false),
    ("row_diff_exclude_columns", true),
    ("row_diff_tolerances", true),
    ("row_diff_sample_rows", false),
    ("row_diff_sample_seed", false),
    ("tags", true),
    ("pre_hooks", false),
    ("post_hooks", false),
];
const TAGS_KEY: &str = "tags";

/// `project_defaults_to_mapping`: each set default of `defaults`, with `tags` as a new list.
pub(crate) fn project_defaults_mapping<'py>(
    defaults: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let values: Bound<'py, PyDict> = PyDict::new(defaults.py());
    for (key, falsy_unset) in DEFAULT_KEYS {
        let value: Bound<'py, PyAny> = native_field(defaults, key)?;
        let unset: bool = if falsy_unset {
            !value.is_truthy()?
        } else {
            value.is_none()
        };
        if unset {
            continue;
        }
        if key == TAGS_KEY {
            values.set_item(
                key,
                PyList::new(
                    defaults.py(),
                    value.try_iter()?.collect::<PyResult<Vec<_>>>()?,
                )?,
            )?;
        } else {
            values.set_item(key, value)?;
        }
    }
    Ok(values)
}
