use crate::maintained_refresh::_helpers::target_lag::parse_target_lag;
use crate::maintained_refresh::main::maintained_settings_drift::maintained_settings_drift;
use crate::maintained_refresh::main::reinitialised_readers::reinitialised_readers;
use crate::maintained_refresh::main::render_target_lag::render_target_lag;
use crate::maintained_refresh::models::RefreshLag;
use crate::maintained_refresh::tests::helpers::{live, read, settings};
use crate::maintained_refresh::tests::test_types::{
    DriftTestCase, ReinitialiseTestCase, RenderLagTestCase, TargetLagTestCase,
};

#[test]
fn given_snowflake_target_lag_text_when_parsing_then_lag_is_read_in_seconds() {
    let test_cases = [
        TargetLagTestCase {
            description: "lag set by readers",
            text: "DOWNSTREAM",
            expected_lag: Some(RefreshLag::Downstream),
        },
        TargetLagTestCase {
            description: "one minute, singular",
            text: "1 minute",
            expected_lag: Some(RefreshLag::Seconds(60)),
        },
        TargetLagTestCase {
            description: "hours, plural and padded",
            text: " 2 hours ",
            expected_lag: Some(RefreshLag::Seconds(7_200)),
        },
        TargetLagTestCase {
            description: "seconds",
            text: "90 seconds",
            expected_lag: Some(RefreshLag::Seconds(90)),
        },
        TargetLagTestCase {
            description: "days",
            text: "1 day",
            expected_lag: Some(RefreshLag::Seconds(86_400)),
        },
        TargetLagTestCase {
            description: "an unknown unit",
            text: "1 fortnight",
            expected_lag: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            parse_target_lag(test_case.text),
            test_case.expected_lag,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_refresh_lag_when_rendering_then_largest_exact_unit_is_used() {
    let test_cases = [
        RenderLagTestCase {
            description: "lag set by readers",
            lag: RefreshLag::Downstream,
            expected_sql: "DOWNSTREAM",
        },
        RenderLagTestCase {
            description: "a whole hour",
            lag: RefreshLag::Seconds(3_600),
            expected_sql: "'1 hour'",
        },
        RenderLagTestCase {
            description: "ninety minutes stays in minutes",
            lag: RefreshLag::Seconds(5_400),
            expected_sql: "'90 minutes'",
        },
        RenderLagTestCase {
            description: "two days",
            lag: RefreshLag::Seconds(172_800),
            expected_sql: "'2 days'",
        },
        RenderLagTestCase {
            description: "seconds that are not whole minutes",
            lag: RefreshLag::Seconds(61),
            expected_sql: "'61 seconds'",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            render_target_lag(test_case.lag),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_live_dynamic_table_when_comparing_settings_then_only_real_drift_alters() {
    let test_cases = [
        DriftTestCase {
            description: "the same lag written in another unit",
            desired: settings(RefreshLag::Seconds(3_600), None),
            live: live("60 minutes", "TRANSFORM_WH"),
            expected_drift: false,
        },
        DriftTestCase {
            description: "a shorter lag",
            desired: settings(RefreshLag::Seconds(300), None),
            live: live("1 hour", "TRANSFORM_WH"),
            expected_drift: true,
        },
        DriftTestCase {
            description: "a lag now set by readers",
            desired: settings(RefreshLag::Downstream, None),
            live: live("1 hour", "TRANSFORM_WH"),
            expected_drift: true,
        },
        DriftTestCase {
            description: "an unset refresh warehouse keeps the existing one",
            desired: settings(RefreshLag::Downstream, None),
            live: live("DOWNSTREAM", "ANY_WH"),
            expected_drift: false,
        },
        DriftTestCase {
            description: "a warehouse matched without case",
            desired: settings(RefreshLag::Downstream, Some("transform_wh")),
            live: live("DOWNSTREAM", "TRANSFORM_WH"),
            expected_drift: false,
        },
        DriftTestCase {
            description: "a different warehouse",
            desired: settings(RefreshLag::Downstream, Some("reporting_wh")),
            live: live("DOWNSTREAM", "TRANSFORM_WH"),
            expected_drift: true,
        },
        DriftTestCase {
            description: "an unreadable live lag is set again",
            desired: settings(RefreshLag::Seconds(60), None),
            live: live("whenever", "TRANSFORM_WH"),
            expected_drift: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            maintained_settings_drift(&test_case.desired, &test_case.live),
            test_case.expected_drift,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_planned_rebuilds_when_checking_maintained_readers_then_reinitialised_ones_are_named() {
    let test_cases = [
        ReinitialiseTestCase {
            description: "a maintained model reading a rebuilt table",
            reads: vec![
                read("orders", false, true, &[]),
                read("daily_totals", true, false, &["orders"]),
            ],
            expected_readers: vec![("daily_totals", "orders")],
        },
        ReinitialiseTestCase {
            description: "an upstream kept in place, such as a merged incremental",
            reads: vec![
                read("orders", false, false, &[]),
                read("daily_totals", true, false, &["orders"]),
            ],
            expected_readers: vec![],
        },
        ReinitialiseTestCase {
            description: "a recreated maintained model reinitialises its maintained readers",
            reads: vec![
                read("clean_orders", true, true, &["orders"]),
                read("daily_totals", true, false, &["clean_orders"]),
            ],
            expected_readers: vec![("daily_totals", "clean_orders")],
        },
        ReinitialiseTestCase {
            description: "a recreated model itself is not reported, and a table reader is ignored",
            reads: vec![
                read("orders", false, true, &[]),
                read("daily_totals", true, true, &["orders"]),
                read("report", false, false, &["orders"]),
            ],
            expected_readers: vec![],
        },
        ReinitialiseTestCase {
            description: "the first replaced upstream by name is named",
            reads: vec![
                read("orders", false, true, &[]),
                read("customers", false, true, &[]),
                read("daily_totals", true, false, &["orders", "customers"]),
            ],
            expected_readers: vec![("daily_totals", "customers")],
        },
    ];
    for test_case in test_cases {
        let readers: Vec<(String, String)> = reinitialised_readers(&test_case.reads);
        let named: Vec<(&str, &str)> = readers
            .iter()
            .map(|(reader, upstream)| (reader.as_str(), upstream.as_str()))
            .collect();

        assert_eq!(
            named, test_case.expected_readers,
            "{}",
            test_case.description
        );
    }
}
