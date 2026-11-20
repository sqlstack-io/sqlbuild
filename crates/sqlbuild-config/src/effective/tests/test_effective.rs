use crate::effective::errors::EffectiveConfigError;
use crate::effective::main::default_connection_name::default_connection_name;
use crate::effective::main::effective_concurrency::effective_concurrency;
use crate::effective::main::effective_connection::effective_connection;
use crate::effective::main::effective_ignore_audit_failures::effective_ignore_audit_failures;
use crate::effective::main::effective_settings::effective_settings;
use crate::effective::main::effective_target_name::effective_target_name;
use crate::effective::main::effective_vars::effective_vars;
use crate::effective::main::merged_target::merged_target;
use crate::effective::models::{
    AuditFailureLayers, ConcurrencyLayers, ConnectionChoice, ConnectionLayers, FieldValue,
    TargetChoice, TargetLayer,
};
use crate::effective::tests::helpers::{entries, field_value, local_layer, project_layer};
use crate::effective::tests::test_types::{
    AuditFailureTestCase, ConcurrencyTestCase, ConnectionTestCase, DefaultConnectionTestCase,
    MergeTestCase, MessageTestCase, Pairs, TargetNameTestCase, UnionTestCase,
};
use crate::effective::types::Entries;

#[test]
fn given_target_names_when_choosing_then_cli_beats_local_beats_project_default() {
    let test_cases = [
        TargetNameTestCase {
            description: "the CLI selection wins",
            selected: Some("ci"),
            local: Some("dev"),
            default: Some("prod"),
            expected_result: Ok(Some("ci")),
        },
        TargetNameTestCase {
            description: "the local target beats the project default",
            selected: None,
            local: Some("dev"),
            default: Some("prod"),
            expected_result: Ok(Some("dev")),
        },
        TargetNameTestCase {
            description: "nothing selects a target",
            selected: None,
            local: None,
            default: None,
            expected_result: Ok(None),
        },
        TargetNameTestCase {
            description: "an undefined name is rejected",
            selected: Some("qa"),
            local: None,
            default: None,
            expected_result: Err(EffectiveConfigError::UnknownTarget("qa")),
        },
    ];
    for test_case in test_cases {
        let choice: TargetChoice<&str> = TargetChoice {
            selected: test_case.selected,
            local: test_case.local,
            default: test_case.default,
            defined: vec!["ci", "dev", "prod"],
        };

        assert_eq!(
            effective_target_name(choice),
            test_case.expected_result,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_connection_layers_when_merging_then_later_layers_override_in_place() {
    let test_cases = [
        ConnectionTestCase {
            description: "no target: project then local connection",
            target_name: None,
            project_named: &[],
            local_named: &[],
            expected_result: Ok(&[("host", 9), ("port", 1), ("user", 9)]),
        },
        ConnectionTestCase {
            description: "a named connection applies project then local entries before the target",
            target_name: Some("warehouse"),
            project_named: &[("warehouse", &[("port", 2), ("role", 2)])],
            local_named: &[("warehouse", &[("role", 3)])],
            expected_result: Ok(&[
                ("host", 9),
                ("port", 2),
                ("role", 3),
                ("db", 4),
                ("user", 9),
            ]),
        },
        ConnectionTestCase {
            description: "an undefined named connection is rejected",
            target_name: Some("missing"),
            project_named: &[("warehouse", &[])],
            local_named: &[],
            expected_result: Err(EffectiveConfigError::UnknownConnection("missing")),
        },
    ];
    for test_case in test_cases {
        let named = |pairs: &'static [(&'static str, Pairs)]| -> Entries<&str, Entries<&str, i64>> {
            pairs
                .iter()
                .map(|(name, pairs)| (*name, entries(pairs)))
                .collect()
        };
        let layers: ConnectionLayers<&str, i64> = ConnectionLayers {
            project: entries(&[("host", 1), ("port", 1)]),
            target: test_case
                .target_name
                .map(|name| (Some(name), entries(&[("db", 4)]))),
            project_named: named(test_case.project_named),
            local_named: named(test_case.local_named),
            local: entries(&[("host", 9), ("user", 9)]),
        };

        assert_eq!(
            effective_connection(layers),
            test_case.expected_result.map(entries),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_named_connections_when_target_selects_none_then_only_one_is_the_default() {
    let test_cases = [
        DefaultConnectionTestCase {
            description: "a target that selects a connection keeps it",
            selected: true,
            project_names: &["a", "b"],
            local_names: &[],
            expected_result: Ok(None),
        },
        DefaultConnectionTestCase {
            description: "the same name in both files is one connection",
            selected: false,
            project_names: &["a"],
            local_names: &["a"],
            expected_result: Ok(Some("a")),
        },
        DefaultConnectionTestCase {
            description: "several names are ambiguous and listed in sorted order",
            selected: false,
            project_names: &["b"],
            local_names: &["a"],
            expected_result: Err(EffectiveConfigError::AmbiguousConnection {
                target: "dev",
                names: vec!["a", "b"],
            }),
        },
    ];
    for test_case in test_cases {
        let choice: ConnectionChoice<&str> = ConnectionChoice {
            target: "dev",
            selected: test_case.selected,
            project_names: test_case.project_names.to_vec(),
            local_names: test_case.local_names.to_vec(),
        };

        assert_eq!(
            default_connection_name(choice),
            test_case.expected_result,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_local_target_when_merging_then_set_fields_override_by_their_rule() {
    let test_cases = [
        MergeTestCase {
            description: "an empty local target keeps project scalars and maps",
            local: &[],
            expected_result: &[
                ("database", || FieldValue::Scalar(Some(3))),
                ("connection", || FieldValue::Map(Some(vec![("project", 0)]))),
                ("max_duration_seconds", || FieldValue::Scalar(Some(20))),
            ],
        },
        MergeTestCase {
            description: "a local scalar overrides and maps union with local entries last",
            local: &[("database", 30), ("vars", 7)],
            expected_result: &[
                ("database", || FieldValue::Scalar(Some(30))),
                ("vars", || {
                    FieldValue::Map(Some(vec![("project", 2), ("local", 7)]))
                }),
                ("schema", || FieldValue::Scalar(Some(4))),
            ],
        },
        MergeTestCase {
            description: "a local max duration decides its seconds even when unset",
            local: &[("max_duration", 5)],
            expected_result: &[
                ("max_duration", || FieldValue::Scalar(Some(5))),
                ("max_duration_seconds", || FieldValue::Scalar(None)),
            ],
        },
        MergeTestCase {
            description: "a local retention empties the unset per-materialization retention",
            local: &[("time_travel_retention", 8)],
            expected_result: &[
                ("time_travel_retention", || FieldValue::Scalar(Some(8))),
                ("time_travel_retention_by_materialization", || {
                    FieldValue::Map(Some(Vec::new()))
                }),
            ],
        },
        MergeTestCase {
            description: "a local per-materialization retention clears the project retention",
            local: &[("time_travel_retention_by_materialization", 6)],
            expected_result: &[
                ("time_travel_retention", || FieldValue::Scalar(None)),
                ("time_travel_retention_by_materialization", || {
                    FieldValue::Map(Some(vec![("local", 6)]))
                }),
            ],
        },
    ];
    for test_case in test_cases {
        let local: TargetLayer<&str, i64> = local_layer(test_case.local);

        let merged: TargetLayer<&str, i64> = merged_target(project_layer(), Some(&local));

        let values: Vec<(&str, FieldValue<&str, i64>)> = test_case
            .expected_result
            .iter()
            .map(|(name, _)| (*name, field_value(&merged, name)))
            .collect();
        let expected: Vec<(&str, FieldValue<&str, i64>)> = test_case
            .expected_result
            .iter()
            .map(|(name, value)| (*name, value()))
            .collect();
        assert_eq!(
            values,
            expected,
            "{} {:?}",
            test_case.description,
            test_case.expected_result.len()
        );
    }
}

#[test]
fn given_layers_when_merging_vars_and_settings_then_dict_update_order_holds() {
    let test_cases = [UnionTestCase {
        description: "keys keep first position and take the last value",
        layers: &[&[("a", 1), ("b", 1)], &[("c", 2)], &[("a", 3)]],
        expected_result: &[("a", 3), ("b", 1), ("c", 2)],
    }];
    for test_case in test_cases {
        let layers: Vec<&[(&str, i64)]> = test_case.layers.to_vec();

        assert_eq!(
            effective_vars(&layers),
            entries(test_case.expected_result),
            "{}",
            test_case.description
        );
        assert_eq!(
            effective_settings(
                entries(&[("a", 1), ("b", 1)]),
                &[("a", 5), ("b", 5)],
                &["b"]
            ),
            entries(&[("a", 1), ("b", 5)]),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_ambiguous_connection_when_rendering_then_names_join_into_the_python_message() {
    let test_cases = [MessageTestCase {
        description: "names are inserted unchanged between the fixed text",
        names: &["a", "b"],
        expected_message: "targets.dev does not set connection and several named connections are \
                           defined (a, b); set connection = \"<name>\" on [targets.dev] in \
                           sqlbuild_project.toml or sqlbuild_local.toml",
    }];
    for test_case in test_cases {
        let error: EffectiveConfigError<&str> = EffectiveConfigError::AmbiguousConnection {
            target: "dev",
            names: test_case.names.to_vec(),
        };

        assert_eq!(
            error.message(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_concurrency_layers_when_resolving_then_cli_beats_target_beats_settings() {
    let test_cases = [
        ConcurrencyTestCase {
            description: "the command line beats the target",
            requested: Some(2),
            target: Some(8),
            settings: 16,
            expected_limit: 2,
        },
        ConcurrencyTestCase {
            description: "the target beats the settings",
            requested: None,
            target: Some(8),
            settings: 4,
            expected_limit: 8,
        },
        ConcurrencyTestCase {
            description: "the settings apply without a target value",
            requested: None,
            target: None,
            settings: 16,
            expected_limit: 16,
        },
    ];

    for test_case in test_cases {
        let resolved = effective_concurrency(&ConcurrencyLayers {
            requested: test_case.requested,
            target: test_case.target,
            settings: test_case.settings,
        });
        assert_eq!(
            resolved.limit, test_case.expected_limit,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_audit_failure_layers_when_resolving_then_request_beats_target_beats_settings() {
    let test_cases = [
        AuditFailureTestCase {
            description: "nothing set keeps audit failures blocking",
            requested: None,
            target: None,
            settings: false,
            expected_ignored: false,
        },
        AuditFailureTestCase {
            description: "the project setting applies without a target or flag",
            requested: None,
            target: None,
            settings: true,
            expected_ignored: true,
        },
        AuditFailureTestCase {
            description: "the target turns it off over the project setting",
            requested: None,
            target: Some(false),
            settings: true,
            expected_ignored: false,
        },
        AuditFailureTestCase {
            description: "the target turns it on over the project default",
            requested: None,
            target: Some(true),
            settings: false,
            expected_ignored: true,
        },
        AuditFailureTestCase {
            description: "the flag or job setting turns it on over the target",
            requested: Some(true),
            target: Some(false),
            settings: false,
            expected_ignored: true,
        },
        AuditFailureTestCase {
            description: "the negated flag or a job's false turns it off over the target",
            requested: Some(false),
            target: Some(true),
            settings: true,
            expected_ignored: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            effective_ignore_audit_failures(&AuditFailureLayers {
                requested: test_case.requested,
                target: test_case.target,
                settings: test_case.settings,
            }),
            test_case.expected_ignored,
            "{}",
            test_case.description
        );
    }
}
