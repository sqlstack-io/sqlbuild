"""Carrier pickup client."""

from python.providers.carrier_api.constants import MINIMUM_CHARGE_CENTS

from sqlbuild.providers import Provider

_CENTS_PER_UNIT: int = 100


class CarrierApi(Provider):
    """Client for the carrier pickup API."""

    endpoint: str = "https://carrier.example.com"

    def minimum_charge(self) -> float:
        """Return the minimum pickup charge in currency units."""
        return MINIMUM_CHARGE_CENTS / _CENTS_PER_UNIT
