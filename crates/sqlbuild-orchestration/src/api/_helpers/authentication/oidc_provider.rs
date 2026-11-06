//! OIDC uses verified HTTPS, bounded bodies and the library's signed issuer/audience/nonce checks.

use std::time::Duration;

use openidconnect::core::{CoreClient, CoreProviderMetadata};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, IssuerUrl, OAuth2TokenResponse,
    RedirectUrl, TokenResponse,
};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::api::constants::{HTTPS_SCHEME, OIDC_RESPONSE_BYTES};
use crate::api::main::parse_origin::parse_origin;
use crate::api::models::{OidcAuthentication, OidcRuntime};
use crate::api::types::{OidcClient, OidcIdToken};
use crate::credentials::models::{BuiltinRole, RoleKey, StoredOidcFlow};
use crate::store::errors::ControlError;

pub(crate) fn configure(mut settings: OidcAuthentication) -> Result<OidcRuntime, ControlError> {
    settings.origin = parse_origin(&settings.origin, "OIDC")?;
    let issuer = https_url(&settings.issuer)?;
    if issuer.query().is_some() || issuer.fragment().is_some() {
        return Err(ControlError::Invalid(
            "OIDC issuer cannot contain a query or fragment".into(),
        ));
    }
    if settings.client_id.trim().is_empty() || settings.identities.keys().any(String::is_empty) {
        return Err(ControlError::Invalid(
            "OIDC requires a nonempty client ID and nonempty bound subjects".into(),
        ));
    }
    validate_group_roles(&settings)?;
    let client_secret = settings
        .client_secret_env
        .as_ref()
        .map(|name| {
            let value = std::env::var(name).map_err(|_| {
                ControlError::Invalid(
                    "OIDC client-secret environment variable is unavailable".into(),
                )
            })?;
            if value.is_empty() {
                return Err(ControlError::Invalid(
                    "OIDC client secret must not be empty".into(),
                ));
            }
            Ok(ClientSecret::new(value))
        })
        .transpose()?;
    let mut http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10));
    if let Some(path) = &settings.certificate_authority {
        let bytes = std::fs::read(path).map_err(|_| {
            ControlError::Invalid("could not read OIDC certificate authority".into())
        })?;
        let certificates = reqwest::Certificate::from_pem_bundle(&bytes)
            .map_err(|_| ControlError::Invalid("invalid OIDC PEM certificate authority".into()))?;
        if certificates.is_empty() {
            return Err(ControlError::Invalid(
                "OIDC certificate authority contains no certificates".into(),
            ));
        }
        for certificate in certificates {
            http = http.add_root_certificate(certificate);
        }
    }
    let configuration_id =
        sha256_hex(&serde_json::to_vec(&settings).map_err(|_| invalid_provider())?);
    Ok(OidcRuntime {
        settings,
        client_secret,
        http: http.build().map_err(|_| invalid_provider())?,
        configuration_id,
    })
}

pub(crate) async fn client(runtime: &OidcRuntime) -> Result<OidcClient, ControlError> {
    let http = |request| request_provider(runtime.http.clone(), request);
    let metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(runtime.settings.issuer.clone()).map_err(|_| invalid_provider())?,
        &http,
    )
    .await
    .map_err(|_| invalid_provider())?;
    https_url(metadata.authorization_endpoint().as_str())?;
    https_url(
        metadata
            .token_endpoint()
            .ok_or_else(invalid_provider)?
            .as_str(),
    )?;
    Ok(CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(runtime.settings.client_id.clone()),
        runtime.client_secret.clone(),
    )
    .set_redirect_uri(
        RedirectUrl::new(format!(
            "{}/api/auth/oidc/callback",
            runtime.settings.origin
        ))
        .map_err(|_| invalid_provider())?,
    ))
}

/// Re-reads the signed ID token with every claim kept, so the configured groups claim survives.
pub(crate) async fn verify_code(
    runtime: &OidcRuntime,
    code: AuthorizationCode,
    flow: StoredOidcFlow,
) -> Result<(String, Vec<String>), ControlError> {
    let client = client(runtime).await?;
    let http = |request| request_provider(runtime.http.clone(), request);
    let tokens = client
        .exchange_code(code)
        .map_err(|_| invalid_provider())?
        .set_pkce_verifier(flow.pkce_verifier)
        .request_async(&http)
        .await
        .map_err(|_| invalid_provider())?;
    let token: OidcIdToken = tokens
        .id_token()
        .ok_or_else(invalid_provider)?
        .to_string()
        .parse()
        .map_err(|_| invalid_provider())?;
    let verifier = client.id_token_verifier();
    let claims = token
        .claims(&verifier, &flow.nonce)
        .map_err(|_| invalid_provider())?;
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            tokens.access_token(),
            token.signing_alg().map_err(|_| invalid_provider())?,
            token
                .signing_key(&verifier)
                .map_err(|_| invalid_provider())?,
        )
        .map_err(|_| invalid_provider())?;
        if actual != *expected {
            return Err(invalid_provider());
        }
    }
    let groups = runtime
        .settings
        .groups_claim
        .as_ref()
        .and_then(|claim| claims.additional_claims().0.get(claim))
        .and_then(claim_groups)
        .unwrap_or_default();
    Ok((claims.subject().as_str().to_owned(), groups))
}

/// A claim that is not a list of nonempty strings, such as an overage pointer, gives no groups.
fn claim_groups(value: &serde_json::Value) -> Option<Vec<String>> {
    value.as_array()?.iter().map(group_name).collect()
}

fn group_name(value: &serde_json::Value) -> Option<String> {
    Some(value.as_str()?)
        .filter(|group| !group.is_empty())
        .map(str::to_owned)
}

fn validate_group_roles(settings: &OidcAuthentication) -> Result<(), ControlError> {
    if !settings.group_roles.is_empty()
        && settings.groups_claim.as_ref().is_none_or(String::is_empty)
    {
        return Err(ControlError::Invalid(
            "OIDC group roles require a nonempty groups_claim".into(),
        ));
    }
    for mapping in &settings.group_roles {
        let scoped = mapping.target.is_some() || matches!(mapping.role, RoleKey::Custom(_));
        let blank = |text: &str| text.trim().is_empty();
        if blank(&mapping.group)
            || mapping.role == RoleKey::Builtin(BuiltinRole::Admin)
            || matches!(&mapping.role, RoleKey::Custom(key) if blank(key))
            || mapping.target.as_deref().is_some_and(blank)
            || (scoped && mapping.location_id.is_none())
        {
            return Err(ControlError::Invalid(format!(
                "OIDC group {:?} needs a non-admin role, and a location for a target or custom role",
                mapping.group
            )));
        }
    }
    Ok(())
}

async fn request_provider(
    client: reqwest::Client,
    request: openidconnect::HttpRequest,
) -> Result<openidconnect::HttpResponse, ControlError> {
    if request.uri().scheme_str() != Some("https") {
        return Err(invalid_provider());
    }
    let mut response = client
        .execute(reqwest::Request::try_from(request).map_err(|_| invalid_provider())?)
        .await
        .map_err(|_| invalid_provider())?;
    let status = response.status();
    let headers = response.headers().clone();
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| invalid_provider())? {
        if chunk.len() > OIDC_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err(invalid_provider());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut result = openidconnect::HttpResponse::new(bytes);
    *result.status_mut() = status;
    *result.headers_mut() = headers;
    Ok(result)
}

fn https_url(value: &str) -> Result<reqwest::Url, ControlError> {
    let url = reqwest::Url::parse(value).map_err(|_| invalid_provider())?;
    if url.scheme() != HTTPS_SCHEME
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ControlError::Invalid(
            "OIDC endpoints require HTTPS URLs without credentials".into(),
        ));
    }
    Ok(url)
}

fn invalid_provider() -> ControlError {
    ControlError::Invalid("OIDC identity-provider validation failed".into())
}
