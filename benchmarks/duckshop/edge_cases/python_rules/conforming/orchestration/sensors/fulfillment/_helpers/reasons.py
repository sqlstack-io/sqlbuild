"""Skip reasons for the fulfillment sensors."""


def prefixed(reason: str) -> str:
    """Prefix a skip reason with its domain."""
    return f"fulfillment: {reason}"
