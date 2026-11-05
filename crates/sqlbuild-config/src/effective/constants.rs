//! Every `TargetConfig` field and how a local target overrides it.

use crate::effective::models::{MergeRule, TargetField};

const RETENTION: &[&str] = &[
    "time_travel_retention",
    "time_travel_retention_by_materialization",
];
const LIMITS: Option<&str> = Some("execution_limits");
const CLONE: Option<&str> = Some("clone");

/// `TargetConfig`'s fields in declaration order, nested dataclass fields under their group.
pub(crate) const TARGET_FIELDS: &[TargetField] = &[
    TargetField {
        group: None,
        name: "connection",
        rule: MergeRule::Union,
    },
    TargetField {
        group: None,
        name: "connection_name",
        rule: MergeRule::Overlay {
            decided_by: &["connection_name"],
        },
    },
    TargetField {
        group: None,
        name: "vars",
        rule: MergeRule::Union,
    },
    TargetField {
        group: None,
        name: "database",
        rule: MergeRule::Overlay {
            decided_by: &["database"],
        },
    },
    TargetField {
        group: None,
        name: "schema",
        rule: MergeRule::Overlay {
            decided_by: &["schema"],
        },
    },
    TargetField {
        group: None,
        name: "loader_schema",
        rule: MergeRule::Overlay {
            decided_by: &["loader_schema"],
        },
    },
    TargetField {
        group: None,
        name: "defer_sources_to",
        rule: MergeRule::Overlay {
            decided_by: &["defer_sources_to"],
        },
    },
    TargetField {
        group: None,
        name: "defer_clone_from",
        rule: MergeRule::Overlay {
            decided_by: &["defer_clone_from"],
        },
    },
    TargetField {
        group: CLONE,
        name: "allow_as_clone_origin",
        rule: MergeRule::Overlay {
            decided_by: &["allow_as_clone_origin"],
        },
    },
    TargetField {
        group: CLONE,
        name: "allow_as_clone_destination",
        rule: MergeRule::Overlay {
            decided_by: &["allow_as_clone_destination"],
        },
    },
    TargetField {
        group: None,
        name: "compile_cache",
        rule: MergeRule::Overlay {
            decided_by: &["compile_cache"],
        },
    },
    TargetField {
        group: None,
        name: "time_travel_retention",
        rule: MergeRule::Overlay {
            decided_by: RETENTION,
        },
    },
    TargetField {
        group: None,
        name: "time_travel_retention_by_materialization",
        rule: MergeRule::ReplaceOrEmpty {
            decided_by: RETENTION,
        },
    },
    TargetField {
        group: None,
        name: "owns_time_travel_retention_namespace",
        rule: MergeRule::Overlay {
            decided_by: &["owns_time_travel_retention_namespace"],
        },
    },
    TargetField {
        group: None,
        name: "default_table_type",
        rule: MergeRule::Overlay {
            decided_by: &["default_table_type"],
        },
    },
    TargetField {
        group: None,
        name: "table_type_downgrade",
        rule: MergeRule::Overlay {
            decided_by: &["table_type_downgrade"],
        },
    },
    TargetField {
        group: None,
        name: "time_travel_retention_decrease",
        rule: MergeRule::Overlay {
            decided_by: &["time_travel_retention_decrease"],
        },
    },
    TargetField {
        group: None,
        name: "missing_migration_origin",
        rule: MergeRule::Overlay {
            decided_by: &["missing_migration_origin"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "max_models",
        rule: MergeRule::Overlay {
            decided_by: &["max_models"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "max_duration",
        rule: MergeRule::Overlay {
            decided_by: &["max_duration"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "max_duration_seconds",
        rule: MergeRule::Overlay {
            decided_by: &["max_duration"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "remediation",
        rule: MergeRule::Overlay {
            decided_by: &["remediation"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "test_timeout",
        rule: MergeRule::Overlay {
            decided_by: &["test_timeout"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "test_timeout_seconds",
        rule: MergeRule::Overlay {
            decided_by: &["test_timeout"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "scenario_timeout",
        rule: MergeRule::Overlay {
            decided_by: &["scenario_timeout"],
        },
    },
    TargetField {
        group: LIMITS,
        name: "scenario_timeout_seconds",
        rule: MergeRule::Overlay {
            decided_by: &["scenario_timeout"],
        },
    },
    TargetField {
        group: Some("diff"),
        name: "max_full_rows",
        rule: MergeRule::Overlay {
            decided_by: &["max_full_rows"],
        },
    },
    TargetField {
        group: Some("warehouses"),
        name: "build",
        rule: MergeRule::Overlay {
            decided_by: &["build"],
        },
    },
    TargetField {
        group: Some("warehouses"),
        name: "query",
        rule: MergeRule::Overlay {
            decided_by: &["query"],
        },
    },
    TargetField {
        group: None,
        name: "concurrency",
        rule: MergeRule::Overlay {
            decided_by: &["concurrency"],
        },
    },
    TargetField {
        group: None,
        name: "catalog_connection",
        rule: MergeRule::Overlay {
            decided_by: &["catalog_connection"],
        },
    },
];
