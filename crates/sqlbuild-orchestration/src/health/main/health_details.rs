//! What administrators see beside the health report: where the database is, each cause's message, and the failed requests.

use crate::health::models::{
    ComponentDetails, ComponentFailure, ComponentState, HealthComponent, HealthDetails,
    ServerHealth,
};

/// The failing components' details, read from memory only; callers decide who may read them.
#[must_use]
pub fn health_details(health: &ServerHealth) -> HealthDetails {
    let state = health.state();
    let host: Option<String> = state
        .target
        .host
        .as_ref()
        .map(|host| match state.target.port {
            Some(port) => format!("{host}:{port}"),
            None => host.clone(),
        });
    let mut components: Vec<ComponentDetails> = vec![ComponentDetails {
        host,
        ..details(HealthComponent::ControlDatabase, &state.control_database)
    }];
    components.extend(
        state
            .scheduler
            .as_ref()
            .map(|scheduler| details(HealthComponent::Scheduler, scheduler)),
    );
    HealthDetails { components }
}

fn details(component: HealthComponent, state: &ComponentState) -> ComponentDetails {
    let failure: Option<&ComponentFailure> =
        state.last_failure.as_ref().filter(|_| state.failing());
    ComponentDetails {
        component,
        location_id: None,
        host: None,
        error_chain: failure
            .map(|failure| failure.chain.clone())
            .unwrap_or_default(),
        request_ids: failure
            .map(|failure| failure.request_ids.iter().copied().collect())
            .unwrap_or_default(),
    }
}
