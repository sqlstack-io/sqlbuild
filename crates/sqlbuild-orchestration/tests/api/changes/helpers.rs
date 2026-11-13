//! Real HTTP event parsing and commit-time fault injection shared by SSE contracts.

use std::collections::BTreeSet;
use std::time::Duration;

use uuid::Uuid;

use crate::api::tests::test_types::{ResetProxy, SseConnection, TestApi};
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) async fn connect(
    client: &reqwest::Client,
    api: &TestApi,
    (location_id, target, bearer): (Uuid, &str, &str),
) -> SseConnection {
    let response = client
        .get(format!(
            "{}/api/locations/{location_id}/targets/{target}/changes",
            api.url
        ))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("open scoped SSE connection");
    opened(response).await
}

/// A person's feed over a browser session cookie; the caller checks the status.
pub(crate) async fn request_person_feed(
    client: &reqwest::Client,
    api: &TestApi,
    (location_id, target, session): (Uuid, &str, &str),
) -> reqwest::Response {
    client
        .get(format!(
            "{}/api/control/locations/{location_id}/targets/{target}/changes",
            api.url
        ))
        .header(reqwest::header::HOST, "control.example.com")
        .header(
            reqwest::header::COOKIE,
            format!("__Host-sqb-session={session}"),
        )
        .send()
        .await
        .expect("open person SSE connection")
}

/// An accepted SSE response, after its initial refetch notice.
pub(crate) async fn opened(response: reqwest::Response) -> SseConnection {
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let mut stream = SseConnection {
        response,
        pending: Vec::new(),
    };
    assert_eq!(
        next_topic(&mut stream).await.as_deref(),
        Some("all"),
        "a reconnect must refetch REST state"
    );
    stream
}

pub(crate) async fn next_topic(stream: &mut SseConnection) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(30), async {
        for _ in 0..4096 {
            for _ in 0..4096 {
                let Some(end) = stream.pending.windows(2).position(|bytes| bytes == b"\n\n") else {
                    break;
                };
                let bytes: Vec<_> = stream.pending.drain(..end + 2).collect();
                let frame = std::str::from_utf8(&bytes).expect("UTF-8 event frame");
                let Some(data) = frame.lines().find_map(|line| line.strip_prefix("data: ")) else {
                    continue;
                };
                assert!(frame.lines().any(|line| line == "event: changed"));
                let body: serde_json::Value =
                    serde_json::from_str(data).expect("typed change notice");
                return Some(body["topic"].as_str().expect("topic").to_owned());
            }
            let chunk = stream
                .response
                .chunk()
                .await
                .expect("SSE body remains readable")?;
            stream.pending.extend_from_slice(&chunk);
        }
        panic!("SSE stream exhausted bounded frame reads")
    })
    .await
    .expect("bounded SSE delivery or shutdown")
}

pub(crate) async fn expect_topics(stream: &mut SseConnection, expected: &[String]) {
    let mut actual = BTreeSet::new();
    for _ in expected {
        assert!(
            actual.insert(next_topic(stream).await.expect("committed change")),
            "transaction-local topics deduplicate"
        );
    }
    assert_eq!(actual, expected.iter().cloned().collect());
}

pub(crate) async fn reject_snapshot_commit(store: &ControlStore) {
    execute_fault_sql(store, (
        "CREATE TABLE sse_commit_parent (id INTEGER PRIMARY KEY); \
         CREATE TABLE sse_commit_guard (id INTEGER REFERENCES sse_commit_parent(id) DEFERRABLE INITIALLY DEFERRED); \
         CREATE TRIGGER sse_reject_snapshot AFTER INSERT ON sqb_snapshots BEGIN INSERT INTO sse_commit_guard VALUES (1); END;",
        "CREATE TABLE sse_commit_parent (id INTEGER PRIMARY KEY); \
         CREATE TABLE sse_commit_guard (id INTEGER REFERENCES sse_commit_parent(id) DEFERRABLE INITIALLY DEFERRED); \
         CREATE FUNCTION sse_reject_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO sse_commit_guard VALUES (1); RETURN NEW; END $$; \
         CREATE TRIGGER sse_reject_snapshot AFTER INSERT ON sqb_snapshots FOR EACH ROW EXECUTE FUNCTION sse_reject_snapshot();"
    )).await;
}

pub(crate) async fn restore_snapshot_commits(store: &ControlStore) {
    execute_fault_sql(store, (
        "DROP TRIGGER sse_reject_snapshot; DROP TABLE sse_commit_guard; DROP TABLE sse_commit_parent;",
        "DROP TRIGGER sse_reject_snapshot ON sqb_snapshots; DROP FUNCTION sse_reject_snapshot(); DROP TABLE sse_commit_guard; DROP TABLE sse_commit_parent;"
    )).await;
}

pub(crate) async fn audit_events(store: &ControlStore) -> i64 {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("inspect audit");
    let (transaction, rows) = query(
        transaction,
        "SELECT count(*) AS events FROM sqb_security_audit",
        vec![],
    )
    .await
    .expect("audit count");
    commit(transaction).await.expect("close inspection");
    rows[0]["events"].as_i64().expect("count")
}

async fn execute_fault_sql(store: &ControlStore, (sqlite, postgres): (&str, &str)) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("fault connection")
            .execute_batch(sqlite)
            .expect("SQLite commit fault setup");
        return;
    };
    sqlx_core::raw_sql::raw_sql(postgres)
        .execute(pool)
        .await
        .expect("PostgreSQL commit fault setup");
}

/// TCP RST, unlike a PostgreSQL termination message, exercises SQLx's unhandled reset error.
pub(crate) async fn reset_proxy(store: &ControlStore) -> ResetProxy {
    let ControlStore::Postgres(pool) = store else {
        panic!("PostgreSQL fixture")
    };
    let options = pool.connect_options().as_ref().clone();
    let upstream = (options.get_host().to_owned(), options.get_port());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy socket");
    let port = listener.local_addr().expect("proxy address").port();
    let (reset, resets) = tokio::sync::watch::channel(0_u64);
    let (shutdown, mut stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        for _ in 0..10_000 {
            tokio::select! {
                accepted = listener.accept() => {
                    let (mut downstream, _) = accepted.expect("accept database connection");
                    let upstream = upstream.clone();
                    let mut reset = resets.clone();
                    reset.borrow_and_update();
                    connections.spawn(async move {
                        let mut upstream = tokio::net::TcpStream::connect(upstream).await.expect("connect real PostgreSQL");
                        downstream.set_zero_linger().expect("reset rather than EOF on drop");
                        tokio::select! {
                            _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream) => {},
                            _ = reset.changed() => {},
                        }
                    });
                },
                Some(result) = connections.join_next(), if !connections.is_empty() => { result.expect("proxy connection task"); },
                _ = &mut stopped => return,
            }
        }
        panic!("database proxy exhausted bounded connection events");
    });
    let store = ControlStore::Postgres(
        sqlx_postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect_with(
                options
                    .host("127.0.0.1")
                    .port(port)
                    .ssl_mode(sqlx_postgres::PgSslMode::Disable),
            )
            .await
            .expect("connect through reset proxy"),
    );
    ResetProxy {
        store,
        reset,
        shutdown,
        task,
    }
}
