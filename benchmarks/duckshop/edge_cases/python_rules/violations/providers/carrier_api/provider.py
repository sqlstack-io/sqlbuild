"""Carrier pickup client."""

from python.inventory.restock.main import restock
from sqlbuild.providers import Provider


def pickup_label(sku: str) -> str:
    """Return the pickup label of one product."""
    restock(sku)
    return sku


class CarrierApi(Provider):
    """Client for the carrier pickup API."""

    endpoint: str = "https://carrier.example.com"
