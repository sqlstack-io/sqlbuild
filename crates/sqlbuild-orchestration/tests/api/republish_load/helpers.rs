//! A large synthetic project republished while people watch its node states and a load balancer polls `/livez`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use reqwest::header;
use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::changes_helpers::{opened, request_person_feed};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::node_states_helpers::orders_snapshot;
use crate::api::tests::test_types::RepublishLoad;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// Set in the child process that runs the measurement alone, so no other test's memory shows in its peak.
const CHILD_VARIABLE: &str = "SQLBUILD_TEST_REPUBLISH_LOAD_CHILD";
/// Marks the child's measurement on its standard output.
const MEASUREMENT_MARKER: &str = "republish-load-measurement: ";
/// Concurrent readers of the node states while the republish lands.
const READERS: usize = 4;

/// The republish measured in a child process running only `test`, a test's full path; `None` without PostgreSQL.
pub(crate) fn measured_in_own_process(test: &str, nodes: usize) -> Option<RepublishLoad> {
    let None = std::env::var_os(CHILD_VARIABLE) else {
        return measured_here(nodes);
    };
    std::env::var_os("SQLBUILD_TEST_CONTROL_POSTGRES_URL")?;
    let name = test.split_once("::").map_or(test, |(_, path)| path);
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([name, "--exact", "--nocapture", "--test-threads", "1"])
        .env(CHILD_VARIABLE, "1")
        .output()
        .expect("run the measurement alone");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the measurement failed: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = stdout
        .lines()
        .find_map(|line| {
            line.split_once(MEASUREMENT_MARKER)
                .map(|(_, measured)| measured)
        })
        .expect("the child reports its measurement");
    Some(serde_json::from_str(line).expect("measurement"))
}

/// The measurement in this process, also written to standard output for the parent.
fn measured_here(nodes: usize) -> Option<RepublishLoad> {
    let measured = run(republish_under_load(nodes))?;
    println!(
        "{MEASUREMENT_MARKER}{}",
        serde_json::to_string(&measured).expect("measurement JSON")
    );
    Some(measured)
}

/// Publish a project of `nodes` models while its node states are watched, then republish every definition changed while `/livez` and node-state reads keep coming; freed memory stays resident, so peak memory is measured from before the first publication's reads.
async fn republish_under_load(nodes: usize) -> Option<RepublishLoad> {
    let fixture = open_test_store(Backend::Postgres).await?;
    let store = &fixture.store;
    migrate_store(store).await.expect("migrate");
    let roles = role_fixture(store).await;
    let location = roles.location_id;
    let (first, second) = (
        large_snapshot(location, nodes, 2),
        large_snapshot(location, nodes, 3),
    );
    let api = start_api_with_authentication(
        store,
        ApiAuthentication {
            login: HumanLogin::Password("https://control.example.com".into()),
            ..ApiAuthentication::default()
        },
    )
    .await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .expect("HTTP client");
    let admin = roles.administrator.bearer.clone();
    let states_url = format!(
        "{}/api/control/locations/{location}/targets/development/node-states",
        api.url
    );
    let feed =
        opened(request_person_feed(&client, &api, (location, "development", &admin)).await).await;
    let baseline = reset_peak_memory();
    let first = publish_snapshot(store, &first)
        .await
        .expect("first publication");
    assert!(
        states_show(&client, (&states_url, &admin), first.snapshot_id).await,
        "the first publication's node states load"
    );
    let stop = Arc::new(AtomicBool::new(false));
    let livez = tokio::spawn(time_livez(
        client.clone(),
        format!("{}/livez", api.url),
        stop.clone(),
    ));
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let (client, url, admin, stop) = (
                client.clone(),
                states_url.clone(),
                admin.clone(),
                stop.clone(),
            );
            tokio::spawn(async move {
                for _ in 0_u64.. {
                    let false = stop.load(Ordering::SeqCst) else {
                        break;
                    };
                    snapshot_of_states(&client, &url, &admin).await;
                }
            })
        })
        .collect();
    let (admin_pool, database) = fixture.postgres.clone().expect("PostgreSQL fixture");
    let idle = tokio::spawn(longest_idle_in_transaction(
        admin_pool,
        database,
        stop.clone(),
    ));
    let published = publish_snapshot(store, &second)
        .await
        .expect("republication");
    let caught_up = states_show(&client, (&states_url, &admin), published.snapshot_id).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    stop.store(true, Ordering::SeqCst);
    let (slowest_livez_ms, livez_answers) = livez.await.expect("livez timing");
    for reader in readers {
        reader.await.expect("node-state reader");
    }
    let longest_idle_in_transaction_ms = idle.await.expect("idle sampling");
    let peak_growth_bytes = peak_memory().saturating_sub(baseline);
    let compressed_bytes = compressed_definitions(store, published.snapshot_id).await;
    drop(feed);
    stop_api(api).await;
    close_test_store(fixture).await;
    Some(RepublishLoad {
        nodes,
        compressed_bytes,
        peak_growth_bytes,
        slowest_livez_ms,
        livez_answers,
        longest_idle_in_transaction_ms,
        caught_up,
    })
}

/// A project of `nodes` chained models whose descriptions are hash text, which compresses about as poorly as real documentation; `generation` changes every one.
fn large_snapshot(location: Uuid, nodes: usize, generation: i64) -> SnapshotInput {
    let mut input = orders_snapshot(location, &[]);
    input.generation = generation;
    for index in 0..nodes {
        let name = format!("orders_{index:04}");
        let text = |part: usize| -> String {
            (0..6)
                .map(|piece| sha256_hex(format!("{generation}/{name}/{part}/{piece}").as_bytes()))
                .collect()
        };
        let columns: Vec<Value> = (0..8)
            .map(|column| json!({"name": format!("column_{column}"), "description": text(column + 1)}))
            .collect();
        let deps: Vec<String> = (index > 0)
            .then(|| format!("model:orders_{:04}", index - 1))
            .into_iter()
            .collect();
        input.nodes.insert(
            format!("model:{name}"),
            SnapshotNode {
                kind: "model".into(),
                definition: json!({"name": name, "description": text(0), "columns": columns, "deps": deps,
                    "destination": {"database": null, "schema": "main", "name": name}}),
                raw_sql: None,
                compiled_sql: None,
                python_source: None,
            },
        );
    }
    input
}

/// Whether the node states show `snapshot_id` within three minutes, read every 100 ms as a person watching them.
async fn states_show(
    client: &reqwest::Client,
    (url, session): (&str, &str),
    snapshot_id: Uuid,
) -> bool {
    for _ in 0..1800 {
        let Some(true) = snapshot_of_states(client, url, session)
            .await
            .map(|shown| shown == snapshot_id)
        else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        return true;
    }
    false
}

/// The snapshot the node states are for, read as a person watching them; `None` before they load.
async fn snapshot_of_states(client: &reqwest::Client, url: &str, session: &str) -> Option<Uuid> {
    let response = client
        .get(url)
        .header(header::HOST, "control.example.com")
        .header(header::COOKIE, format!("__Host-sqb-session={session}"))
        .send()
        .await
        .ok()?;
    let states: Value = response.json().await.ok()?;
    states["snapshot_id"].as_str()?.parse().ok()
}

/// `/livez`'s slowest answer in milliseconds and how many answers came, polled every 20 ms until `stop`.
async fn time_livez(client: reqwest::Client, url: String, stop: Arc<AtomicBool>) -> (u64, u64) {
    let (mut slowest, mut answers) = (0_u64, 0_u64);
    for _ in 0_u64.. {
        let false = stop.load(Ordering::SeqCst) else {
            break;
        };
        let started = Instant::now();
        let answered = client
            .get(&url)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success());
        slowest = slowest.max(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
        answers += u64::from(answered);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    (slowest, answers)
}

/// The longest any session of `database` sat idle in a transaction, in milliseconds, sampled every 50 ms until `stop`.
async fn longest_idle_in_transaction(
    admin: sqlx_postgres::PgPool,
    database: String,
    stop: Arc<AtomicBool>,
) -> u64 {
    let mut longest = 0_f64;
    for _ in 0_u64.. {
        let false = stop.load(Ordering::SeqCst) else {
            break;
        };
        let idle: Option<f64> = sqlx_core::query_scalar::query_scalar(
            "SELECT MAX(EXTRACT(EPOCH FROM (clock_timestamp() - state_change)) * 1000)::float8 \
             FROM pg_stat_activity WHERE datname=$1 AND state='idle in transaction'",
        )
        .bind(&database)
        .fetch_one(&admin)
        .await
        .expect("sample sessions");
        longest = longest.max(idle.unwrap_or_default());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    longest as u64
}

/// The snapshot's definitions' compressed bytes, as the store holds them.
async fn compressed_definitions(
    store: &crate::store::models::ControlStore,
    snapshot_id: Uuid,
) -> u64 {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(
        transaction,
        "SELECT SUM(octet_length(d.body))::bigint AS bytes FROM sqb_snapshot_nodes n \
         JOIN sqb_node_definitions d ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1",
        vec![Parameter::Id(snapshot_id)],
    )
    .await
    .expect("compressed size");
    commit(transaction).await.expect("close read");
    rows[0]["bytes"].as_u64().unwrap_or_default()
}

/// Reset this process's peak resident memory to what it holds now, and return that, in bytes.
fn reset_peak_memory() -> u64 {
    std::fs::write("/proc/self/clear_refs", "5").expect("reset peak resident memory");
    peak_memory()
}

/// This process's peak resident memory since the last reset, in bytes.
fn peak_memory() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("process status");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|value| {
            value
                .trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<u64>()
                .ok()
        })
        .expect("peak resident memory")
        * 1024
}
