//! A local test's failure where the warehouse would not fail, as the test runner reports it.

use std::collections::HashMap;

use crate::run_on::models::LocalRefusal;

/// The test named by `fields["test"]` and its refusal, from `category`, `construct` and `reason`.
#[must_use]
pub fn read_local_refusal(mut fields: HashMap<String, String>) -> (String, LocalRefusal) {
    let mut take = |key: &str| fields.remove(key).unwrap_or_default();
    (
        take("test"),
        LocalRefusal {
            category: take("category"),
            construct: take("construct"),
            reason: take("reason"),
            width: 1,
            ..LocalRefusal::default()
        },
    )
}
