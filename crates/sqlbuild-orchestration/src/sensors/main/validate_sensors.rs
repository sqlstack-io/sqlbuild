//! Native publication validates every sensor before any snapshot pointer moves.

use crate::declarations::models::Definitions;
use crate::sensors::models::SensorCondition;
use crate::store::errors::ControlError;

pub(crate) fn validate_sensors(definitions: &Definitions) -> Result<(), ControlError> {
    for (name, sensor) in &definitions.sensors {
        let SensorCondition::PlatformEvent { kinds } = &sensor.when;
        if name.trim().is_empty()
            || sensor.description.trim().is_empty()
            || kinds.is_empty()
            || (!definitions.jobs.contains_key(&sensor.job_key)
                && !definitions.groups.contains_key(&sensor.job_key))
        {
            return Err(ControlError::Invalid(format!(
                "sensor {name} needs a description, a known job and nonempty event kinds"
            )));
        }
    }
    Ok(())
}
