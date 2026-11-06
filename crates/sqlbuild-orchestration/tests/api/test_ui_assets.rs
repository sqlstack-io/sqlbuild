//! The embedded UI answers client routes with its index and never shadows API or auth paths.

use axum::Router;
use axum::routing::get;

use crate::api::_helpers::resources::ui_assets::serve_ui;
use crate::api::models::UiAsset;
use crate::api::tests::test_types::UiAssetTestCase;
use crate::store::tests::store_helpers::run;

#[test]
fn given_embedded_ui_when_requesting_pages_assets_and_api_paths_then_only_ui_paths_serve_ui_files()
{
    static ASSETS: &[UiAsset] = &[
        UiAsset {
            path: "/index.html",
            body: b"<html>shell</html>",
        },
        UiAsset {
            path: "/_app/immutable/entry/start.js",
            body: b"start()",
        },
        UiAsset {
            path: "/theme.js",
            body: b"theme()",
        },
    ];
    let test_cases = [
        UiAssetTestCase {
            description: "root serves the shell",
            method: reqwest::Method::GET,
            path: "/",
            expected_status: 200,
            expected_body: "<html>shell</html>",
            expected_cache: Some("no-cache"),
        },
        UiAssetTestCase {
            description: "client route falls back to the shell",
            method: reqwest::Method::GET,
            path: "/runs/01a1b089",
            expected_status: 200,
            expected_body: "<html>shell</html>",
            expected_cache: Some("no-cache"),
        },
        UiAssetTestCase {
            description: "hashed asset is immutable",
            method: reqwest::Method::GET,
            path: "/_app/immutable/entry/start.js",
            expected_status: 200,
            expected_body: "start()",
            expected_cache: Some("public, max-age=31536000, immutable"),
        },
        UiAssetTestCase {
            description: "unhashed asset revalidates",
            method: reqwest::Method::GET,
            path: "/theme.js",
            expected_status: 200,
            expected_body: "theme()",
            expected_cache: Some("no-cache"),
        },
        UiAssetTestCase {
            description: "missing file is not the shell",
            method: reqwest::Method::GET,
            path: "/_app/immutable/entry/gone.js",
            expected_status: 404,
            expected_body: "",
            expected_cache: None,
        },
        UiAssetTestCase {
            description: "unknown API path is not the shell",
            method: reqwest::Method::GET,
            path: "/api/unknown",
            expected_status: 404,
            expected_body: "",
            expected_cache: None,
        },
        UiAssetTestCase {
            description: "bare API prefix is not the shell",
            method: reqwest::Method::GET,
            path: "/api",
            expected_status: 404,
            expected_body: "",
            expected_cache: None,
        },
        UiAssetTestCase {
            description: "unknown auth path is not the shell",
            method: reqwest::Method::GET,
            path: "/auth/unknown",
            expected_status: 404,
            expected_body: "",
            expected_cache: None,
        },
        UiAssetTestCase {
            description: "writes never reach the shell",
            method: reqwest::Method::POST,
            path: "/runs",
            expected_status: 404,
            expected_body: "",
            expected_cache: None,
        },
        UiAssetTestCase {
            description: "API routes keep precedence",
            method: reqwest::Method::GET,
            path: "/api/openapi.json",
            expected_status: 200,
            expected_body: "api",
            expected_cache: None,
        },
    ];
    run(async {
        let application = serve_ui(
            Router::new().route("/api/openapi.json", get(|| async { "api" })),
            ASSETS,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("socket");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move { axum::serve(listener, application).await });
        let client = reqwest::Client::new();
        for test_case in test_cases {
            let response = client
                .request(test_case.method, format!("{url}{}", test_case.path))
                .send()
                .await
                .expect("UI request");
            let headers = response.headers().clone();
            let header = |name: &str| {
                headers
                    .get(name)
                    .map(|value| value.to_str().expect("ASCII header").to_owned())
            };
            assert_eq!(
                response.status().as_u16(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                header("cache-control").as_deref(),
                test_case.expected_cache,
                "{}",
                test_case.description
            );
            assert_eq!(
                header("content-security-policy").is_some(),
                test_case.expected_cache.is_some(),
                "{}",
                test_case.description
            );
            assert_eq!(
                response.text().await.expect("body"),
                test_case.expected_body,
                "{}",
                test_case.description
            );
        }
        server.abort();
    });
}
