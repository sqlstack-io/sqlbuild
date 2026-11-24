use crate::cost_report::models::{CostResource, CostRun, CostSummary};

pub(super) fn resource(name: &str, usd: &str, bytes_scanned: i64) -> CostResource {
    CostResource {
        resource_type: "model".to_owned(),
        resource_name: name.to_owned(),
        warehouse_name: "TRANSFORM_WH".to_owned(),
        warehouse_size: "X-SMALL".to_owned(),
        warehouse_type: Some("STANDARD".to_owned()),
        cluster_number: Some(1),
        query_count: 2,
        attributed_seconds: "12.345".to_owned(),
        bytes_scanned,
        estimated_compute_credits: "0.0034291666666666666666666667".to_owned(),
        estimated_usd: usd.to_owned(),
    }
}

pub(super) fn run(completed_at: &str, resources: Vec<CostResource>) -> CostRun {
    CostRun {
        run_id: "run-1".to_owned(),
        adapter_name: "snowflake".to_owned(),
        target_name: Some("dev".to_owned()),
        build_status: "success".to_owned(),
        started_at: "2026-09-01T10:00:00+02:00".to_owned(),
        completed_at: completed_at.to_owned(),
        cost: CostSummary {
            status: "complete".to_owned(),
            usd_per_credit: "3.00".to_owned(),
            rate_source: "default".to_owned(),
            resources,
            query_count: 4,
            attributed_seconds: "24.69".to_owned(),
            bytes_scanned: 1536,
            estimated_compute_credits: "0.0068583333333333333333333334".to_owned(),
            estimated_usd: "0.0205750000000000000000000002".to_owned(),
            expected_statement_count: 4,
            observed_statement_count: 4,
            missing_statement_count: 0,
            skipped_statement_count: 0,
            source: "snowflake_information_schema_query_history".to_owned(),
            method: "equal_share_visible_query_busy_time_v1".to_owned(),
            limitations: Vec::new(),
            message: None,
        },
        version: 1,
        had_executable_work: Some(true),
    }
}

/// `run` priced from `source`.
pub(crate) fn priced(mut run: CostRun, source: &str) -> CostRun {
    run.cost.rate_source = source.to_owned();
    run
}
