//! Why a sensor's `timeout_seconds` can't be published.

use crate::sensors::constants::MAX_SENSOR_TIMEOUT_SECONDS;
use crate::sensors::models::SensorDefinition;

/// The refusal of `sensor`'s timeout when it is longer than `MAX_SENSOR_TIMEOUT_SECONDS`.
#[must_use]
pub fn sensor_timeout_error(name: &str, sensor: &SensorDefinition) -> Option<String> {
    let timeout = sensor.timeout_seconds.get();
    (timeout > MAX_SENSOR_TIMEOUT_SECONDS).then(|| {
        format!(
            "sensor {name}: timeout_seconds is {timeout}; the most is {MAX_SENSOR_TIMEOUT_SECONDS}, because a running sensor holds its location's other sensors, plans and sync until it returns"
        )
    })
}
