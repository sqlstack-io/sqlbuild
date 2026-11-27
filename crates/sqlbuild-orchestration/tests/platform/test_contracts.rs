//! Platform contracts compiled within the platform owner.

#[path = "helpers.rs"]
mod helpers;
#[path = "test_compile_failure.rs"]
mod test_compile_failure;
#[path = "test_failing_locations.rs"]
mod test_failing_locations;
#[path = "test_failing_sinks.rs"]
mod test_failing_sinks;
#[path = "test_failing_syncs.rs"]
mod test_failing_syncs;
#[path = "test_installation.rs"]
mod test_installation;
#[path = "test_location_state.rs"]
mod test_location_state;
#[path = "test_platform_event_reads.rs"]
mod test_platform_event_reads;
#[path = "test_problems.rs"]
mod test_problems;
#[path = "test_provider_secrets.rs"]
mod test_provider_secrets;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_watchdog.rs"]
mod test_watchdog;
#[path = "test_webhook.rs"]
mod test_webhook;
