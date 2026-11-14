from dataclasses import dataclass
from decimal import Decimal


@dataclass(frozen=True)
class ResourceCostPublicationTestCase:
    description: str
    usd_per_credit_is_default: bool
    expected_run_usd: Decimal | None
