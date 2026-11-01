//! The provider's discovered authorization and token endpoints retain their typed availability.

use openidconnect::{EndpointMaybeSet, EndpointNotSet, EndpointSet};

pub(crate) type OidcClient = openidconnect::core::CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;
