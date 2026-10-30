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
        loop {
            if client
                .get(format!("{url}/api/openapi.json"))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                break;
            }
            assert!(!task.is_finished(), "API failed during startup");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
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
