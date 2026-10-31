use std::time::Duration;

use sqlbuild_orchestration::api::main::serve_api::serve_api;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::test_types::TestApi;

pub(crate) async fn start_api(store: &ControlStore) -> TestApi {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("isolated API socket");
    let url = format!("http://{}", listener.local_addr().expect("API address"));
    let (shutdown, signal) = tokio::sync::oneshot::channel();
    let store = store.clone();
    let task = tokio::spawn(serve_api(store, listener, async {
        let _ = signal.await;
    }));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("HTTP client");
    tokio::time::timeout(Duration::from_secs(5), async {
        for _ in 0..250 {
            assert!(!task.is_finished(), "API failed during startup");
            tokio::time::sleep(Duration::from_millis(20)).await;
            let Ok(response) = client
                .get(format!("{url}/api/openapi.json"))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            else {
                continue;
            };
            drop(response);
            return;
        }
        panic!("API exhausted readiness attempts");
    })
    .await
    .expect("API becomes ready");
    TestApi {
        url,
        shutdown,
        task,
    }
}

pub(crate) async fn stop_api(api: TestApi) {
    api.shutdown.send(()).expect("request API shutdown");
    tokio::time::timeout(Duration::from_secs(2), api.task)
        .await
        .expect("prompt API shutdown")
        .expect("join API server")
        .expect("API server stopped cleanly");
}

pub(crate) async fn read_json(
    client: &reqwest::Client,
    (url, bearer): (&str, &str),
) -> serde_json::Value {
    let response = client
        .get(url)
        .bearer_auth(bearer)
        .send()
        .await
        .expect("HTTP read");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().await.expect("typed JSON response")
}
