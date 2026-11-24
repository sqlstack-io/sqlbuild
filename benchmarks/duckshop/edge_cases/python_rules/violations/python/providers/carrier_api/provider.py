"""Carrier pickup client."""

import python.nodes.report

from sqlbuild.providers import Provider


def pickup_label(sku: str) -> str:
    """Return the pickup label of one product."""
    return f"{sku} {python.report.__name__}"


class CarrierApi(Provider):
    """Client for the carrier pickup API."""

    endpoint: str = "https://carrier.example.com"
