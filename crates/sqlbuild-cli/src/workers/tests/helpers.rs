//! A claimed local worker without a child process, for supervision bookkeeping.

use crate::workers::models::ClaimedWorker;

/// A claimed worker that leads process group `group`, registered as a launch registers it.
pub(super) fn worker_leading(group: u32) -> ClaimedWorker {
    let claim = serde_json::from_value(serde_json::json!({
        "claim_id": "00000000-0000-0000-0000-000000000001",
        "request": {
            "run_id": "00000000-0000-0000-0000-000000000002",
            "location_id": "00000000-0000-0000-0000-000000000003",
            "target": "development",
            "snapshot_id": "00000000-0000-0000-0000-000000000004",
            "image_id": null,
            "kind": "job",
            "origin": "manual",
            "job_key": "orders",
            "parent_run_id": null,
            "root_run_id": "00000000-0000-0000-0000-000000000002",
            "trigger_id": null,
            "tick_id": null,
            "rerun_of_run_id": null,
            "requested_by": "test",
            "priority": 0,
            "selection": ["model:orders"],
            "write_set": ["model:orders"],
            "settings": {}
        }
    }))
    .expect("a run claim");
    crate::dev_stop::main::stop_switch::stop_switch().register_group(group);
    ClaimedWorker {
        claim,
        child: None,
        stopping: None,
        exit: None,
        launch_failure: None,
        recovered: false,
        group: Some(group),
    }
}
