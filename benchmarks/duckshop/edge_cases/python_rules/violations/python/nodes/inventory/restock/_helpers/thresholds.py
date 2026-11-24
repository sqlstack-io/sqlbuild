"""Read restock thresholds, from the stock service when asked to refresh."""

import json
import urllib.request

_DEFAULT_THRESHOLDS: dict[str, int] = {"north": 10, "south": 5}


def load_thresholds(refresh: bool = False) -> dict[str, int]:
    """Return each warehouse's restock threshold."""
    if refresh:
        return json.load(urllib.request.urlopen("https://example.com/thresholds"))
    return dict(_DEFAULT_THRESHOLDS)
