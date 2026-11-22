"""Skip restocking while the shipment backlog is empty."""

from orchestration.sensors.fulfillment._constants import BACKLOG_REASON
from orchestration.sensors.fulfillment._helpers.reasons import prefixed

from sqlbuild.orchestration import SensorContext, SkipReason, sensor

_REASON_SUFFIX: str = "."


@sensor(
    description="Rebuild shipments when the backlog grows.",
    minimum_interval_seconds=60,
)
def shipment_backlog_sensor(ctx: SensorContext[None]) -> SkipReason:
    return SkipReason(prefixed(BACKLOG_REASON) + _REASON_SUFFIX)
