//! A preview copy holds each node's newest mirrored facts, dated by the stalest required clean sync.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::platform::models::{LocationHealthReport, LocationWorkClient};
use crate::warehouse_metadata::models::{PreviewCopy, PreviewCopyRead};

const NODE: &str = "preview_copy_orders";
/// A relation with more executions than a build reads recently; read-back can still need them all.
const BUSY_NODE: &str = "preview_copy_customers";

async fn post(client: &Client, (url, bearer): (&str, &str), body: Value) {
    let response = client
        .post(url)
        .bearer_auth(bearer)
        .json(&body)
        .send()
        .await
        .expect("metadata batch");
    assert_eq!(response.status(), StatusCode::OK, "{body}");
}

fn fingerprint(run_id: &str, definition: &str, ts: &str) -> Value {
    json!({"state_database": null, "state_schema": "main", "node_type": "model", "node_name": NODE,
        "target_database": null, "target_schema": "main", "target_name": NODE, "run_id": run_id,
        "definition_hash": format!("definition-{run_id}"), "version_hash": format!("version-{run_id}"),
        "schema_fingerprint": "id:INTEGER", "definition": definition, "metadata_json": "{}", "ts": ts})
}

fn consumption(execution_id: &str, (phase, run_id): (&str, &str), recorded_at: &str) -> Value {
    json!({"state_database": null, "state_schema": "main", "execution_id": execution_id, "phase": phase,
        "run_id": run_id, "node_type": "model", "node_name": NODE, "target_database": null,
        "target_schema": "main", "target_name": NODE, "version_hash": "version", "input_count": 0,
        "input_kind": null, "input_name": null, "input_version": null, "recorded_at": recorded_at})
}

fn observation(
    id: &str,
    (kind, node, value): (&str, &str, &str),
    (run_id, at): (&str, &str),
) -> Value {
    json!({"state_database": null, "state_schema": "main", "observation_id": id, "kind": kind,
        "node": node, "cursor_type": (kind == "cursor_max").then_some("integer"), "value": value,
        "run_id": run_id, "observed_at": at})
}

fn sync_report(session: Uuid, table: &str, failure: Option<&str>) -> LocationHealthReport {
    schema_sync_report(session, ("main", table), failure)
}

fn schema_sync_report(
    session: Uuid,
    (schema, table): (&str, &str),
    failure: Option<&str>,
) -> LocationHealthReport {
    serde_json::from_value(json!({
        "event_id": Uuid::now_v7(), "session_id": session, "target": "preview",
        "health": {"kind": "warehouse_sync", "generation": 1, "location": {"database": null, "schema": schema},
            "table": table, "failure": failure, "records_sent": 1, "elapsed_millis": 1},
    }))
    .expect("sync health")
}

/// The preview target's copy: newest fingerprint, succeeded run and observation per node, and `synced_at`.
pub(crate) async fn preview_copy_scope(
    outbound: &LocationWorkClient,
    (session_id, preview_session): (Uuid, Uuid),
    (client, url, bearer): (&Client, &str, &str),
) {
    let metadata = url.replace("/development/", "/preview/");
    let batch = |kind: &str, records: Vec<Value>| json!({"kind": kind, "session_id": preview_session, "generation": 1, "records": records});
    post(
        client,
        (&metadata, bearer),
        batch(
            "fingerprints",
            vec![
                fingerprint("copy-run-2", "SELECT 2 AS id", "2026-03-01T11:00:00Z"),
                fingerprint("copy-run-1", "SELECT 1 AS id", "2026-03-01T10:00:00Z"),
            ],
        ),
    )
    .await;
    post(
        client,
        (&metadata, bearer),
        batch(
            "consumption",
            vec![
                consumption(
                    "copy-execution-1",
                    ("succeeded", "copy-run-1"),
                    "2026-03-01T10:00:00Z",
                ),
                consumption(
                    "copy-execution-2",
                    ("succeeded", "copy-run-2"),
                    "2026-03-01T11:00:00Z",
                ),
                consumption(
                    "copy-execution-3",
                    ("started", "copy-run-3"),
                    "2026-03-01T12:00:00Z",
                ),
            ],
        ),
    )
    .await;
    let mut busy: Vec<Value> = Vec::new();
    for execution in 1..=10 {
        for (phase, hour, minute) in [("started", execution, 0), ("succeeded", execution, 30)] {
            let mut row = consumption(
                &format!("busy-execution-{execution:02}"),
                (phase, &format!("busy-run-{execution:02}")),
                &format!("2026-03-02T{hour:02}:{minute:02}:00Z"),
            );
            row["node_name"] = json!(BUSY_NODE);
            row["target_name"] = json!(BUSY_NODE);
            busy.push(row);
        }
    }
    post(client, (&metadata, bearer), batch("consumption", busy)).await;
    post(
        client,
        (&metadata, bearer),
        batch(
            "observations",
            vec![
                observation(
                    "copy-observation-2",
                    ("cursor_max", NODE, "2"),
                    ("copy-run-2", "2026-03-01T11:00:01Z"),
                ),
                observation(
                    "copy-observation-1",
                    ("cursor_max", NODE, "1"),
                    ("copy-run-1", "2026-03-01T10:00:01Z"),
                ),
                observation(
                    "copy-observation-3",
                    ("source_columns", "raw_copy", "{}"),
                    ("copy-run-1", "2026-03-01T10:00:02Z"),
                ),
            ],
        ),
    )
    .await;
    let mut catalog = batch(
        "catalog",
        vec![json!({"state_database": null, "state_schema": "main",
        "name": NODE, "relation_type": "table", "columns": [{"name": "id", "data_type": "INTEGER"}]})],
    );
    catalog["scan"] = json!({"scan_id": Uuid::now_v7(), "location": {"database": null, "schema": "main"}, "complete": true});
    post(client, (&metadata, bearer), catalog).await;

    let read = PreviewCopyRead {
        session_id: preview_session,
        generation: 1,
        consumption: false,
    };
    let copy = || async {
        outbound
            .preview_copy("preview", &read)
            .await
            .expect("preview copy")
    };
    let ours = |copy: &PreviewCopy| {
        (
            copy.fingerprints
                .iter()
                .filter(|f| f.node_name == NODE)
                .map(|f| (f.run_id.clone(), f.definition.clone()))
                .collect::<Vec<_>>(),
            copy.latest_runs
                .iter()
                .filter(|r| r.node_name == NODE)
                .map(|r| r.run_id.clone())
                .collect::<Vec<_>>(),
            copy.observations
                .iter()
                .filter(|o| o.observation_id.starts_with("copy-"))
                .map(|o| (o.kind.clone(), o.value.clone(), o.run_id.clone()))
                .collect::<Vec<_>>(),
            copy.relations
                .iter()
                .filter(|r| r.name == NODE)
                .map(|r| r.columns.len())
                .collect::<Vec<_>>(),
        )
    };
    let unsynced = copy().await;
    let older_location: Value = client
        .post(format!("{metadata}/preview-copy"))
        .bearer_auth(bearer)
        .json(&json!({"session_id": preview_session, "generation": 1}))
        .send()
        .await
        .expect("copy for an older location")
        .json()
        .await
        .expect("copy body");
    assert!(
        unsynced.consumption.is_empty() && older_location.get("consumption").is_none(),
        "a read that does not ask for consumption never carries the field, which older locations reject"
    );
    let with_consumption = outbound
        .preview_copy(
            "preview",
            &PreviewCopyRead {
                consumption: true,
                ..read.clone()
            },
        )
        .await
        .expect("preview copy with consumption");
    let busy: Vec<(String, String)> = with_consumption
        .consumption
        .iter()
        .filter(|row| row.target_name.as_deref() == Some(BUSY_NODE))
        .map(|row| {
            (
                row.execution_id.clone().unwrap_or_default(),
                row.phase.clone().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        busy.len(),
        20,
        "every mirrored row, beyond the newest eight executions, so read-back finds older ones: {busy:?}"
    );
    assert_eq!(unsynced.synced_at, None, "nothing synced cleanly yet");
    assert_eq!(
        ours(&unsynced),
        (
            vec![("copy-run-2".into(), "SELECT 2 AS id".into())],
            vec!["copy-run-2".into()],
            vec![
                ("cursor_max".into(), "2".into(), "copy-run-2".into()),
                ("source_columns".into(), "{}".into(), "copy-run-1".into()),
            ],
            vec![1],
        ),
        "newest fingerprint with its definition, newest succeeded run, newest observation per kind"
    );

    let fingerprints_clean = sync_report(preview_session, "fingerprints", None);
    for report in [
        sync_report(preview_session, "catalog", None),
        fingerprints_clean.clone(),
        sync_report(preview_session, "catalog", None),
        sync_report(preview_session, "consumption", Some("read")),
    ] {
        assert!(
            outbound
                .report_health(&report)
                .await
                .expect("report health")
        );
    }
    assert_eq!(
        copy().await.synced_at,
        None,
        "consumption never synced cleanly"
    );
    for report in [
        sync_report(preview_session, "consumption", None),
        sync_report(preview_session, "fingerprints", Some("read")),
        sync_report(preview_session, "observations", Some("read")),
    ] {
        assert!(
            outbound
                .report_health(&report)
                .await
                .expect("report health")
        );
    }
    assert_eq!(
        copy().await.synced_at,
        None,
        "every table of main synced, but products builds into reporting, which never synced"
    );
    for table in ["catalog", "fingerprints", "consumption"] {
        assert!(
            outbound
                .report_health(&schema_sync_report(
                    preview_session,
                    ("reporting", table),
                    None
                ))
                .await
                .expect("report health")
        );
    }
    let health_url = format!(
        "{}/health",
        metadata.split("/targets/").next().expect("location URL")
    );
    let observations: Vec<Value> = client
        .get(&health_url)
        .bearer_auth(bearer)
        .send()
        .await
        .expect("health")
        .json()
        .await
        .expect("health reply");
    let fingerprints_at = observations
        .iter()
        .find(|observation| observation["report"]["event_id"] == json!(fingerprints_clean.event_id))
        .and_then(|observation| observation["received_at"].as_str())
        .map(|at| at.parse::<chrono::DateTime<chrono::Utc>>().expect("time"));
    assert_eq!(
        copy().await.synced_at,
        fingerprints_at,
        "the stalest required table's newest clean sync; failures and observations do not count"
    );

    let development = outbound
        .preview_copy(
            "development",
            &PreviewCopyRead {
                session_id,
                generation: 1,
                consumption: false,
            },
        )
        .await
        .expect("development copy");
    assert_eq!(
        ours(&development),
        (vec![], vec![], vec![], vec![]),
        "another target's mirror never leaks"
    );

    let refused = |session: Uuid, generation: i64, target: &str| {
        let endpoint = format!(
            "{}/preview-copy",
            metadata.replace("/preview/", &format!("/{target}/"))
        );
        let request = client
            .post(endpoint)
            .bearer_auth(bearer)
            .json(&json!({"session_id": session, "generation": generation}));
        async move { request.send().await.expect("refused copy").status() }
    };
    assert_eq!(
        refused(preview_session, 2, "preview").await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        refused(session_id, 1, "preview").await,
        StatusCode::BAD_REQUEST
    );
}
