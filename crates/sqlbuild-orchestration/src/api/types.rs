//! Typed OIDC endpoints, and ID tokens that keep every claim for the configured groups claim.

use openidconnect::{EndpointMaybeSet, EndpointNotSet, EndpointSet};

pub(crate) type OidcClient = openidconnect::core::CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub(crate) type OidcIdToken = openidconnect::IdToken<
    crate::api::models::OidcExtraClaims,
    openidconnect::core::CoreGenderClaim,
    openidconnect::core::CoreJweContentEncryptionAlgorithm,
    openidconnect::core::CoreJwsSigningAlgorithm,
>;
