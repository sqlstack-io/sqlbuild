"""Carrier pickup client."""

from python.utils.currency import amounts
from sqlbuild.providers import Provider

_MINIMUM_CHARGE_CENTS: int = 250


class CarrierApi(Provider):
    """Client for the carrier pickup API."""

    endpoint: str = "https://carrier.example.com"

    def minimum_charge(self) -> str:
        """Return the minimum pickup charge as text."""
        return amounts.cents_text(_MINIMUM_CHARGE_CENTS)
