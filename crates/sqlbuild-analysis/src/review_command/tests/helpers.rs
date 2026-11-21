use std::collections::BTreeMap;

use crate::review_command::models::{
    AddedNode, ChangedNode, ColumnEvidence, DownstreamPolicy, IncompleteCheck, IncrementalStart,
    NodeComparison, NodeReport, ReviewBaseline, ReviewExample, ReviewFailure, ReviewFinding,
    ReviewGraph, ReviewNode, ReviewPlan, ReviewReport, ReviewSchemaReport, SelectedToken,
    ValidationCounts,
};

fn node(name: &str, kind: &str, critical: bool) -> (String, ReviewNode) {
    (
        name.to_owned(),
        ReviewNode {
            name: name.to_owned(),
            kind: kind.to_owned(),
            critical,
            materialized: "table".to_owned(),
            unique_key: vec!["order_id".to_owned()],
            has_hooks: name == "order_totals",
            ..ReviewNode::default()
        },
    )
}

fn python_node(name: &str, kind: &str) -> (String, ReviewNode) {
    (
        name.to_owned(),
        ReviewNode {
            name: name.to_owned(),
            kind: kind.to_owned(),
            python: true,
            ..ReviewNode::default()
        },
    )
}

/// A waffle shop with a critical `daily_revenue`, a loader-fed `stg_toppings` and a Python asset.
pub(super) fn waffle_shop() -> ReviewGraph {
    let edges: [(&str, &str); 8] = [
        ("waffle_types", "stg_orders"),
        ("stg_orders", "order_totals"),
        ("order_totals", "daily_revenue"),
        ("stg_orders", "order_archive"),
        ("stg_customers", "customer_lifetime"),
        ("daily_revenue", "asset:export_revenue"),
        ("loader:raw_toppings", "stg_toppings"),
        ("stg_toppings", "topping_totals"),
    ];
    let mut downstream: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (upstream, child) in edges {
        downstream
            .entry(upstream.to_owned())
            .or_default()
            .push(child.to_owned());
    }
    ReviewGraph {
        nodes: [
            node("waffle_types", "seed", false),
            node("stg_orders", "model", false),
            node("stg_customers", "model", false),
            node("order_totals", "model", false),
            node("order_archive", "model", false),
            node("daily_revenue", "model", true),
            node("customer_lifetime", "model", false),
            python_node("asset:export_revenue", "asset"),
            python_node("loader:raw_toppings", "loader"),
            node("stg_toppings", "model", false),
            node("topping_totals", "model", true),
        ]
        .into_iter()
        .collect(),
        downstream,
        order: [
            "loader:raw_toppings",
            "waffle_types",
            "stg_customers",
            "stg_orders",
            "stg_toppings",
            "customer_lifetime",
            "order_archive",
            "order_totals",
            "topping_totals",
            "daily_revenue",
            "asset:export_revenue",
        ]
        .iter()
        .map(|name| (*name).to_owned())
        .collect(),
    }
}

pub(super) fn changed(name: &str, reason: &str) -> ChangedNode {
    ChangedNode {
        name: name.to_owned(),
        kind: "model".to_owned(),
        reasons: vec![reason.to_owned()],
        added: false,
    }
}

pub(super) fn token(token: &str, names: &[&str]) -> SelectedToken {
    SelectedToken {
        token: token.to_owned(),
        names: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}

pub(super) fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|name| (*name).to_owned()).collect()
}

fn reported(name: &str, status: &str, comparison: Option<NodeComparison>) -> NodeReport {
    NodeReport {
        name: name.to_owned(),
        kind: "model".to_owned(),
        critical: name == "daily_revenue",
        changed: name == "stg_orders",
        status: status.to_owned(),
        reason: (status == "not_validated").then(|| "not selected (--select)".to_owned()),
        build_status: (status != "not_validated").then(|| "success".to_owned()),
        comparison,
        affected_columns: Vec::new(),
    }
}

fn full(target_rows: i64, branch_rows: i64) -> NodeComparison {
    NodeComparison {
        mode: "full".to_owned(),
        target_relation: Some("prod.order_totals".to_owned()),
        branch_relation: Some("review_20261023t101500z_3f2a9c1_a1b2c3.order_totals".to_owned()),
        target_rows: Some(target_rows),
        branch_rows: Some(branch_rows),
        compared_rows: Some(target_rows),
        unique_key: vec!["order_id".to_owned()],
        unchanged_columns: 2,
        complete: true,
        ..NodeComparison::default()
    }
}

/// A review of a `stg_orders` change whose `order_totals.amount` differs in one row.
pub(super) fn finding_report() -> ReviewReport {
    let mut totals: NodeComparison = full(5, 5);
    totals.columns.push(ColumnEvidence {
        column: "amount".to_owned(),
        metric: "rows differ".to_owned(),
        target: "0".to_owned(),
        branch: "1".to_owned(),
        limit: "exact match".to_owned(),
    });
    let mut revenue: NodeReport = reported("daily_revenue", "ok", Some(full(2, 2)));
    revenue.affected_columns = vec!["total_revenue".to_owned()];
    ReviewReport {
        version: "0.172.0".to_owned(),
        baseline: ReviewBaseline {
            since_ref: "origin/main".to_owned(),
            merge_base: "3f2a9c1d00000000000000000000000000000000".to_owned(),
            head: "a1b2c3d400000000000000000000000000000000".to_owned(),
            against: "prod".to_owned(),
            target: Some("dev".to_owned()),
        },
        duration_seconds: 4.5,
        plan: ReviewPlan {
            policy: DownstreamPolicy::Critical,
            changed: vec![changed("stg_orders", "model SQL changed")],
            validated: names(&["stg_orders", "order_totals", "daily_revenue"]),
            not_validated: Vec::new(),
            critical: names(&["daily_revenue"]),
            added: Vec::new(),
            hooks: Vec::new(),
        },
        changed_declarations: vec![(
            "macro".to_owned(),
            "cents_to_dollars".to_owned(),
            "changed".to_owned(),
        )],
        nodes: vec![
            reported("stg_orders", "ok", Some(full(5, 5))),
            reported("order_totals", "finding", Some(totals)),
            revenue,
            reported("order_archive", "not_validated", None),
        ],
        impact_edges: vec![
            ("stg_orders".to_owned(), "order_totals".to_owned()),
            ("order_totals".to_owned(), "daily_revenue".to_owned()),
            ("stg_orders".to_owned(), "order_archive".to_owned()),
        ],
        findings: vec![ReviewFinding {
            node: "order_totals".to_owned(),
            kind: "values".to_owned(),
            metric: "values differ in 1 of 5 rows".to_owned(),
            column: Some("amount".to_owned()),
            target_value: None,
            branch_value: None,
            limit: "exact match".to_owned(),
            examples: vec![ReviewExample {
                key: "order_id=3".to_owned(),
                target: Some("12.5".to_owned()),
                branch: Some("13.0".to_owned()),
            }],
        }],
        incremental_starts: vec![IncrementalStart {
            node: "order_events".to_owned(),
            method: "full_refresh".to_owned(),
            reason: "the adapter has no zero-copy clone".to_owned(),
        }],
        deferred: names(&["waffle_types"]),
        tests: ValidationCounts {
            selected: 2,
            total: 5,
            passed: 2,
            ..ValidationCounts::default()
        },
        audits: ValidationCounts {
            selected: 3,
            total: 3,
            passed: 3,
            ..ValidationCounts::default()
        },
        schema: Some(ReviewSchemaReport {
            database: None,
            name: "review_20261023t101500z_3f2a9c1_a1b2c3".to_owned(),
            created: true,
            dropped: true,
            error: None,
        }),
        invocation_id: Some("a1b2c3d4e5f6".to_owned()),
        ..ReviewReport::default()
    }
}

/// `finding_report` narrowed to `order_totals`, which added `stg_orders` and ran hooks.
pub(super) fn selected_report() -> ReviewReport {
    let mut report: ReviewReport = finding_report();
    report.plan.added = vec![AddedNode {
        name: "stg_orders".to_owned(),
        required_by: "order_totals".to_owned(),
    }];
    report.plan.hooks = names(&["order_totals"]);
    report
}

/// `finding_report` with no differences and one model left incomplete.
pub(super) fn incomplete_report() -> ReviewReport {
    let mut report: ReviewReport = finding_report();
    report.findings.clear();
    report.nodes[1].status = "built".to_owned();
    report.nodes[1].comparison = None;
    report.incomplete.push(IncompleteCheck {
        node: "order_totals".to_owned(),
        check: "keyed comparison".to_owned(),
        reason: "the model has no unique_key".to_owned(),
    });
    report
}

/// The incomplete review whose schema could not be dropped.
pub(super) fn failed_report() -> ReviewReport {
    let report: ReviewReport = incomplete_report();
    ReviewReport {
        schema: report.schema.clone().map(|schema| ReviewSchemaReport {
            dropped: false,
            error: Some("permission denied".to_owned()),
            ..schema
        }),
        ..report
    }
}

/// The incomplete review with every check completed.
pub(super) fn passed_report() -> ReviewReport {
    ReviewReport {
        incomplete: Vec::new(),
        ..incomplete_report()
    }
}

/// The passing review as a dry run.
pub(super) fn dry_run_report() -> ReviewReport {
    ReviewReport {
        dry_run: true,
        ..passed_report()
    }
}

/// The finding review ended by a `--select` error.
pub(super) fn errored_report() -> ReviewReport {
    ReviewReport {
        error: Some(ReviewFailure {
            code: "C761".to_owned(),
            message: "--select x matches no model or seed".to_owned(),
            help: None,
        }),
        ..finding_report()
    }
}
