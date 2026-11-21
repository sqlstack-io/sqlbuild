//! A saved person session can read a published sensor's events for `sqb sensor try`; reading changes nothing.

use std::path::Path;

use sqlbuild_orchestration::sensors::models::{SensorEventChoice, SensorTrialEvents};
use uuid::Uuid;

use crate::person_login::_helpers::transport;

/// The published sensor and the events `choice` picks, read with the saved session for `server`.
pub(crate) fn read_sensor_events(
    server: (&str, Option<&Path>),
    (location, target, name): (Uuid, &str, &str),
    choice: &SensorEventChoice,
) -> Result<SensorTrialEvents, std::io::Error> {
    transport::with_person_session(server, async |client, credential| {
        transport::sensor_events(client, credential, (location, target, name), choice).await
    })
}
