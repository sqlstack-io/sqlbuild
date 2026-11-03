//! HTTPS identity is verified; redirects cannot forward person credentials to another origin.

use std::path::Path;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderValue};
use sqlbuild_orchestration::credentials::models::SessionIdentity;

use crate::person_login::constants::REQUEST_SECONDS;
use crate::person_login::models::{PersonClient, PersonCredential};

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

pub(crate) fn authorized(
    client: &PersonClient,
    method: reqwest::Method,
    path: &str,
    credential: &PersonCredential,
) -> Result<reqwest::RequestBuilder, std::io::Error> {
    let mut authorization = HeaderValue::from_str(&format!("Session {}", credential.bearer))
        .map_err(|_| std::io::Error::other("stored CLI credential is invalid"))?;
    authorization.set_sensitive(true);
    let mut csrf = HeaderValue::from_str(&credential.csrf_token)
        .map_err(|_| std::io::Error::other("stored CLI credential is invalid"))?;
    csrf.set_sensitive(true);
    Ok(client
        .client
        .request(method, format!("{}{path}", client.origin))
        .header(AUTHORIZATION, authorization)
        .header("x-sqb-csrf", csrf))
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
