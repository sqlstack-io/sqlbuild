use crate::runs::models::{RunEnding, RunNodeCounts};

pub(crate) struct RunStatusTestCase {
    pub(crate) description: &'static str,
    pub(crate) counts: RunNodeCounts,
    pub(crate) ending: RunEnding,
    pub(crate) expected_status: &'static str,
}

pub(crate) struct DistinctRunSchemasTestCase {
    pub(crate) description: &'static str,
    pub(crate) schemas: &'static [(Option<&'static str>, &'static str)],
    pub(crate) default_database: Option<&'static str>,
    pub(crate) expected_schemas: &'static [(Option<&'static str>, &'static str)],
}
