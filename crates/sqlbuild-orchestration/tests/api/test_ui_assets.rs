//! The embedded UI answers client routes with its index and never shadows API or auth paths.

use axum::Router;
use axum::routing::get;

use crate::api::_helpers::resources::ui_assets::serve_ui;
use crate::api::models::UiAsset;
use crate::api::tests::helpers::raw_get;
use crate::api::tests::test_types::{UiAssetTestCase, UiPathTestCase};
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

#[test]
fn given_embedded_ui_when_paths_carry_dot_segments_encodings_or_extra_slashes_then_they_never_reach_ui_files_or_bypass_api_routes()
 {
    static ASSETS: &[UiAsset] = &[
        UiAsset {
            path: "/index.html",
            body: b"<html>shell</html>",
        },
        UiAsset {
            path: "/theme.js",
            body: b"theme()",
        },
    ];
    let test_cases = [
        UiPathTestCase {
            description: "parent segment after the API prefix",
            path: "/api/../theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "parent segment inside a client route",
            path: "/runs/../theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "current segment",
            path: "/./theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "encoded parent segment",
            path: "/runs/%2e%2e/theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "upper-case encoded parent segment",
            path: "/%2E%2E/theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "encoded slash",
            path: "/runs%2ftheme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "encoded backslash",
            path: "/%5ctheme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "double slash before the API",
            path: "//api/openapi.json",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "double slash before an asset",
            path: "//theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "double slash inside a route",
            path: "/runs//x",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "raw backslash",
            path: "/runs\\..\\theme.js",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "raw backslash before the API",
            path: "/\\api/openapi.json",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "fragment sent on an asset path",
            path: "/theme.js#x",
            expected_status: 200,
            expected_body: "theme()",
        },
        UiPathTestCase {
            description: "bare API prefix",
            path: "/api",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "API prefix with a trailing slash",
            path: "/api/",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "auth prefix with a trailing slash",
            path: "/auth/",
            expected_status: 404,
            expected_body: "",
        },
        UiPathTestCase {
            description: "a path that only starts with api is a client route",
            path: "/apix",
            expected_status: 200,
            expected_body: "<html>shell</html>",
        },
        UiPathTestCase {
            description: "a path that only starts with auth is a client route",
            path: "/authx",
            expected_status: 200,
            expected_body: "<html>shell</html>",
        },
        UiPathTestCase {
            description: "query string on an asset",
            path: "/theme.js?v=1",
            expected_status: 200,
            expected_body: "theme()",
        },
        UiPathTestCase {
            description: "query naming the API on a client route",
            path: "/runs?next=/api/x",
            expected_status: 200,
            expected_body: "<html>shell</html>",
        },
        UiPathTestCase {
            description: "API route keeps precedence",
            path: "/api/openapi.json",
            expected_status: 200,
            expected_body: "api",
        },
        UiPathTestCase {
            description: "auth route keeps precedence",
            path: "/auth/login",
            expected_status: 200,
            expected_body: "auth",
        },
    ];
    run(async {
        let application = serve_ui(
            Router::new()
                .route("/api/openapi.json", get(|| async { "api" }))
                .route("/auth/login", get(|| async { "auth" })),
            ASSETS,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("socket");
        let address = listener.local_addr().expect("address").to_string();
        let server = tokio::spawn(async move { axum::serve(listener, application).await });
        for test_case in test_cases {
            let target = address.clone();
            let (status, body) =
                tokio::task::spawn_blocking(move || raw_get(&target, test_case.path))
                    .await
                    .expect("raw request task");
            assert_eq!(
                status, test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(body, test_case.expected_body, "{}", test_case.description);
        }
        server.abort();
    });
}
