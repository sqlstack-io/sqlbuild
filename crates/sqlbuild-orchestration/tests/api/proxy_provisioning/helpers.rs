//! Starts proxy-authenticated APIs and sends requests the way a host-rewriting proxy forwards them.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};

use crate::api::models::{
    ApiAuthentication, HumanLogin, TrustedProxyAuthentication, UnknownProxyUsers,
};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::test_types::TestApi;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) const ORIGIN: &str = "http://control.example.com";
pub(crate) const PUBLIC_HOST: &str = "control.example.com";
pub(crate) const USER_HEADER: &str = "x-example-user";
pub(crate) const NAME_HEADER: &str = "x-example-name";
pub(crate) const LOGOUT_URL: &str = "https://sign-in.example.com/logout";

pub(crate) fn settings(
    trusted_peer: &str,
    unknown_users: UnknownProxyUsers,
) -> TrustedProxyAuthentication {
    TrustedProxyAuthentication {
        origin: ORIGIN.into(),
        trusted_peers: BTreeSet::from([trusted_peer.parse().expect("peer IP")]),
        identities: BTreeMap::new(),
        username_header: "X-Example-User".into(),
        display_name_header: Some("X-Example-Name".into()),
        email_header: None,
        unknown_users,
        proxy_logout_url: Some(LOGOUT_URL.into()),
    }
}

pub(crate) async fn start(store: &ControlStore, settings: TrustedProxyAuthentication) -> TestApi {
    start_api_with_authentication(
        store,
        ApiAuthentication {
            login: HumanLogin::TrustedProxy(settings),
            ..ApiAuthentication::default()
        },
    )
    .await
}

/// What the proxy forwards: the backend address as `Host` and the browser's host in `X-Forwarded-Host`.
pub(crate) fn forwarded(client: &Client, method: reqwest::Method, url: String) -> RequestBuilder {
    client
        .request(method, url)
        .header("x-forwarded-host", PUBLIC_HOST)
}

pub(crate) async fn rows(store: &ControlStore, sql: &str, parameters: Vec<Parameter>) -> Value {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        sql,
        parameters,
    )
    .await
    .expect("query");
    commit(transaction).await.expect("read complete");
    json!(rows)
}
