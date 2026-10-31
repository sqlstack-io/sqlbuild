//! Worker output contract helpers inspect durable bytes on both databases.

use serde_json::json;
use sqlbuild_orchestration::runs::models::OutputPublication;
use sqlbuild_orchestration::store::main::read_recall_view::read_recall_view;
use sqlbuild_orchestration::store::models::ControlStore;
use sqlbuild_orchestration::worker::errors::WorkerError;
use sqlbuild_orchestration::worker::models::WorkerClient;
use sqlbuild_recall::records::models::{OmittedOutput, OutputChunk, OutputStream};
use uuid::Uuid;

pub(crate) fn chunk(sequence: u64, stream: OutputStream, data: &[u8]) -> OutputChunk {
    OutputChunk {
        sequence,
        stream,
        occurred_at: "2026-01-02T03:04:05Z".into(),
        data: data.to_vec(),
        byte_offset: 0,
        dropped_before: 0,
        omitted: None,
    }
}

pub(crate) async fn stored_bytes(store: &ControlStore, invocation: &str) -> Vec<(i64, Vec<u8>)> {
    let sql = "SELECT sequence, data FROM recall_output WHERE invocation_id=$1 ORDER BY sequence";
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!("SQLite store")
        };
        let connection = rusqlite::Connection::open(path).expect("inspect SQLite bytes");
        let mut statement = connection.prepare(sql).expect("output query");
        return statement
            .query_map([invocation], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("stored rows")
            .collect::<Result<_, _>>()
            .expect("decode bytes");
    };
    sqlx_core::query_as::query_as(sql)
        .bind(invocation)
        .fetch_all(pool)
        .await
        .expect("inspect PostgreSQL bytes")
}

pub(crate) async fn output_contract(store: &ControlStore, worker: &WorkerClient, claim_id: Uuid) {
    let invocation = format!("worker/{claim_id}");
    let first = chunk(0, OutputStream::Stdout, b"orders\0\xff\n");
    let warning = chunk(1, OutputStream::Stderr, b"warning\xc3\x28");
    let omission = OutputChunk {
        omitted: Some(OmittedOutput {
            bytes: 43,
            sequences: 2,
        }),
        byte_offset: first.data.len() as u64,
        dropped_before: 1,
        ..chunk(3, OutputStream::Stdout, b"")
    };
    let chunks = [first.clone(), warning.clone(), omission];
    assert_eq!(
        worker
            .publish_output(&chunks)
            .await
            .expect("publish exact bytes"),
        OutputPublication {
            inserted: 3,
            duplicates: 0,
            conflicts: 0
        }
    );
    assert_eq!(
        worker
            .publish_output(&chunks)
            .await
            .expect("retry accepted output"),
        OutputPublication {
            inserted: 0,
            duplicates: 3,
            conflicts: 0
        }
    );
    let mut equivalent = first.clone();
    equivalent.occurred_at = "2026-01-02T04:04:05+01:00".into();
    assert_eq!(
        worker
            .publish_output(&[equivalent])
            .await
            .expect("equivalent timestamp retry")
            .duplicates,
        1
    );
    let mut conflict = first.clone();
    conflict.data = b"different".to_vec();
    assert_eq!(
        worker
            .publish_output(&[conflict])
            .await
            .expect("record retry conflict")
            .conflicts,
        1
    );
    assert_eq!(
        stored_bytes(store, &invocation).await,
        vec![
            (0, first.data.clone()),
            (1, warning.data.clone()),
            (3, vec![])
        ]
    );
    let conflicts = read_recall_view(store, ("conflicts", &invocation), 10)
        .await
        .expect("durable conflicts");
    assert_eq!(conflicts.len(), 1);
    let counts = read_recall_view(store, ("output_invocations", &invocation), 10)
        .await
        .expect("output accounting");
    assert_eq!(counts[0]["stdout_bytes"], json!(first.data.len()));
    assert_eq!(counts[0]["stderr_bytes"], json!(warning.data.len()));
    assert_eq!(counts[0]["truncated_bytes"], json!(43));

    let valid = chunk(4, OutputStream::Stdout, b"must roll back");
    let mut invalid = chunk(5, OutputStream::Stderr, b"invalid timestamp");
    invalid.occurred_at = "not-a-time".into();
    for rejected in [
        vec![valid.clone(), invalid],
        vec![OutputChunk {
            sequence: u64::MAX,
            ..valid.clone()
        }],
        vec![OutputChunk {
            sequence: i64::MAX as u64,
            ..valid.clone()
        }],
        vec![OutputChunk {
            byte_offset: u64::MAX,
            ..valid.clone()
        }],
        vec![OutputChunk {
            dropped_before: u64::MAX,
            ..valid.clone()
        }],
        vec![OutputChunk {
            omitted: Some(OmittedOutput {
                bytes: 1,
                sequences: 0,
            }),
            ..valid.clone()
        }],
        vec![valid.clone(); 257],
        vec![chunk(4, OutputStream::Stdout, &vec![0; 1_048_577])],
    ] {
        assert!(matches!(
            worker.publish_output(&rejected).await,
            Err(WorkerError::Rejected(reqwest::StatusCode::BAD_REQUEST))
        ));
    }
    assert_eq!(
        stored_bytes(store, &invocation).await.len(),
        3,
        "invalid batches are atomic"
    );
    assert_eq!(
        worker
            .publish_output(&vec![first; 256])
            .await
            .expect("chunk count boundary")
            .duplicates,
        256
    );
    let boundary = chunk(4, OutputStream::Stdout, &vec![0; 1_048_576]);
    assert_eq!(
        worker
            .publish_output(&[boundary])
            .await
            .expect("byte count boundary")
            .inserted,
        1
    );
}
