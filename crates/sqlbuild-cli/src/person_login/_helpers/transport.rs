//! HTTPS identity is verified; redirects cannot forward person credentials to another origin.

use std::path::Path;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderValue};
use sqlbuild_orchestration::api::main::parse_origin::parse_origin;
use sqlbuild_orchestration::api::models::{ApiErrorBody, MutationResponse};
use sqlbuild_orchestration::credentials::models::SessionIdentity;
use sqlbuild_orchestration::platform::models::{PersonWorkRequest, RepublishCall, RepublishStatus};
use sqlbuild_orchestration::runs::models::PastRunsRead;
use sqlbuild_orchestration::sensors::models::{SensorEventChoice, SensorTrialEvents};
use sqlbuild_orchestration::snapshots::models::NodeSql;
use uuid::Uuid;

use crate::person_login::_helpers::credentials;
use crate::person_login::constants::REQUEST_SECONDS;
use crate::person_login::models::{Bootstrap, LocationTarget, PersonClient, PersonCredential};

pub(crate) fn connect(origin: &str, ca: Option<&Path>) -> Result<PersonClient, std::io::Error> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(REQUEST_SECONDS))
        .default_headers(
            [(
                reqwest::header::ORIGIN,
                HeaderValue::from_str(origin).map_err(std::io::Error::other)?,
            )]
            .into_iter()
            .collect(),
        );
    if let Some(ca) = ca {
        builder = builder.add_root_certificate(
            reqwest::Certificate::from_pem(&std::fs::read(ca)?)
                .map_err(|_| std::io::Error::other("could not read the trusted CA certificate"))?,
        );
    }
    Ok(PersonClient {
        client: builder
            .build()
            .map_err(|_| std::io::Error::other("could not initialize the HTTPS client"))?,
        origin: origin.into(),
    })
}

/// Runs one request with the saved credential for `server`, failing when nobody is signed in.
pub(crate) fn with_person_session<T>(
    (server, ca): (&str, Option<&Path>),
    request: impl AsyncFnOnce(&PersonClient, &PersonCredential) -> Result<T, std::io::Error>,
) -> Result<T, std::io::Error> {
    let origin = parse_origin(server, "CLI").map_err(std::io::Error::other)?;
    let credential = credentials::load(&origin)?
        .ok_or_else(|| std::io::Error::other("not signed in; run sqb login"))?;
    let client = connect(&origin, ca)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(request(&client, &credential))
}

pub(crate) fn authorized(
    client: &PersonClient,
    method: reqwest::Method,
    path: &str,
    credential: &PersonCredential,
) -> Result<reqwest::RequestBuilder, std::io::Error> {
    let mut authorization = HeaderValue::from_str(&format!("Session {}", credential.bearer))
        .map_err(|_| std::io::Error::other("stored CLI credential is invalid"))?;
    authorization.set_sensitive(true);
    Ok(client
        .client
        .request(method, format!("{}{path}", client.origin))
        .header(AUTHORIZATION, authorization))
}

pub(crate) async fn current(
    client: &PersonClient,
    credential: &PersonCredential,
) -> Result<SessionIdentity, std::io::Error> {
    let response = authorized(
        client,
        reqwest::Method::GET,
        "/api/auth/session",
        credential,
    )?
    .send()
    .await
    .map_err(|_| std::io::Error::other("could not verify the CLI session with the server"))?;
    if !response.status().is_success() {
        return Err(std::io::Error::other(
            "CLI session is unavailable or revoked; run sqb login again",
        ));
    }
    response
        .json()
        .await
        .map_err(|_| std::io::Error::other("server returned an invalid person identity"))
}

pub(crate) async fn revoke(
    client: &PersonClient,
    credential: &PersonCredential,
) -> Result<(), std::io::Error> {
    let response = authorized(
        client,
        reqwest::Method::POST,
        "/api/auth/logout",
        credential,
    )?
    .send()
    .await
    .map_err(|_| std::io::Error::other("remote session revocation could not be confirmed"))?;
    if response.status().is_success() || response.status() == reqwest::StatusCode::UNAUTHORIZED {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "remote session revocation could not be confirmed",
        ))
    }
}

pub(crate) async fn node_sql(
    client: &PersonClient,
    credential: &PersonCredential,
    (location, target, node): (Uuid, &str, &str),
) -> Result<Option<NodeSql>, std::io::Error> {
    let path = target_path(client, (location, target), &["nodes", node, "sql"])?;
    send_json(
        authorized(client, reqwest::Method::GET, &path, credential)?,
        (
            "read published SQL from",
            "SQL inspection",
            "SQL inspection result",
        ),
    )
    .await
}

/// Whether the server queued the request; `false` means this request ID was already queued.
pub(crate) async fn warehouse_sync(
    client: &PersonClient,
    credential: &PersonCredential,
    (location, target): (Uuid, &str),
    reason: &str,
) -> Result<bool, std::io::Error> {
    let path = target_path(client, (location, target), &["warehouse-syncs"])?;
    let request = PersonWorkRequest {
        request_id: Uuid::now_v7(),
        reason: reason.into(),
    };
    send_json::<MutationResponse>(
        authorized(client, reqwest::Method::POST, &path, credential)?.json(&request),
        (
            "request a warehouse sync from",
            "Warehouse sync request",
            "warehouse sync response",
        ),
    )
    .await
    .map(|response| response.applied)
}

/// The republish after `call`; `None` when there is no such request.
pub(crate) async fn republish(
    client: &PersonClient,
    credential: &PersonCredential,
    scope: (Uuid, &str),
    call: RepublishCall<'_>,
) -> Result<Option<RepublishStatus>, std::io::Error> {
    let (request, action) = match call {
        RepublishCall::Request(request) => {
            let path = target_path(client, scope, &["republish"])?;
            let posted = authorized(client, reqwest::Method::POST, &path, credential)?;
            (posted.json(request), "request a republish from")
        }
        RepublishCall::Read { request_id, wait } => {
            let path = target_path(client, scope, &["republish", &request_id.to_string()])?;
            let read = authorized(client, reqwest::Method::GET, &path, credential)?;
            (read.query(&[("wait", wait)]), "read the republish from")
        }
    };
    send_json(request, (action, "Republish request", "republish status")).await
}

/// The published sensor and the events `choice` picks; a refusal carries the server's reason.
pub(crate) async fn sensor_events(
    client: &PersonClient,
    credential: &PersonCredential,
    (location, target, name): (Uuid, &str, &str),
    choice: &SensorEventChoice,
) -> Result<SensorTrialEvents, std::io::Error> {
    let path = target_path(client, (location, target), &["sensors", name, "events"])?;
    let query: [(&str, String); 1] = match choice {
        SensorEventChoice::Recent { limit } => [("limit", limit.to_string())],
        SensorEventChoice::Latest => [("latest", "true".to_owned())],
        SensorEventChoice::Event { event_id } => [("event", event_id.clone())],
        SensorEventChoice::Run { run_id } => [("run", run_id.to_string())],
    };
    let response = authorized(client, reqwest::Method::GET, &path, credential)?
        .query(&query)
        .send()
        .await
        .map_err(|_| std::io::Error::other("could not read sensor events from the server"))?;
    if response.status() == reqwest::StatusCode::CONFLICT {
        let body: ApiErrorBody = response
            .json()
            .await
            .map_err(|_| std::io::Error::other("server returned an invalid error response"))?;
        return Err(std::io::Error::other(body.error));
    }
    if !response.status().is_success() {
        return Err(rejection(response, "Reading sensor events").await);
    }
    response
        .json()
        .await
        .map_err(|_| std::io::Error::other("server returned invalid sensor events"))
}

/// The location targets the signed-in person can view.
pub(crate) async fn location_targets(
    client: &PersonClient,
    credential: &PersonCredential,
) -> Result<Vec<LocationTarget>, std::io::Error> {
    send_json::<Bootstrap>(
        authorized(
            client,
            reqwest::Method::GET,
            "/api/control/bootstrap",
            credential,
        )?,
        ("read locations from", "Location lookup", "location list"),
    )
    .await
    .map(|bootstrap| bootstrap.access.locations)
}

/// Each failure names the action, e.g. "could not read published SQL from the server".
async fn send_json<T: serde::de::DeserializeOwned>(
    request: reqwest::RequestBuilder,
    (unreachable, action, result): (&str, &str, &str),
) -> Result<T, std::io::Error> {
    let response = request
        .send()
        .await
        .map_err(|_| std::io::Error::other(format!("could not {unreachable} the server")))?;
    if !response.status().is_success() {
        return Err(rejection(response, action).await);
    }
    response
        .json()
        .await
        .map_err(|_| std::io::Error::other(format!("server returned an invalid {result}")))
}

/// What each run `ids` names selected, didn't finish and skipped in the location and target, or why one can't be read.
pub(crate) async fn past_runs(
    client: &PersonClient,
    credential: &PersonCredential,
    (location, target): (Uuid, &str),
    ids: &[String],
) -> Result<PastRunsRead, std::io::Error> {
    let path = target_path(client, (location, target), &["selection", "runs"])?;
    let response = authorized(client, reqwest::Method::GET, &path, credential)?
        .query(&[("ids", ids.join(","))])
        .send()
        .await
        .map_err(|_| std::io::Error::other("could not read past runs from the server"))?;
    if !response.status().is_success() {
        return Err(rejection(response, "Reading past runs").await);
    }
    response
        .json()
        .await
        .map_err(|_| std::io::Error::other("server returned invalid past runs"))
}

/// Each segment is percent-encoded, so a target or node key cannot change the route.
fn target_path(
    client: &PersonClient,
    (location, target): (Uuid, &str),
    rest: &[&str],
) -> Result<String, std::io::Error> {
    let mut url = reqwest::Url::parse(&client.origin).map_err(std::io::Error::other)?;
    url.path_segments_mut()
        .map_err(|()| std::io::Error::other("invalid server origin"))?
        .pop_if_empty()
        .extend([
            "api",
            "control",
            "locations",
            &location.to_string(),
            "targets",
            target,
        ])
        .extend(rest);
    Ok(url.path().to_owned())
}

async fn rejection(response: reqwest::Response, action: &str) -> std::io::Error {
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return std::io::Error::other("CLI session is unavailable or revoked; run sqb login again");
    }
    match response.json::<ApiErrorBody>().await {
        Ok(body) => std::io::Error::other(format!(
            "{action} rejected (HTTP {}{})",
            status.as_u16(),
            body.decision_id
                .map_or_else(String::new, |id| format!("; decision {id}"))
        )),
        Err(_) => std::io::Error::other(format!(
            "{action} rejected (HTTP {}); invalid error response",
            status.as_u16()
        )),
    }
}
