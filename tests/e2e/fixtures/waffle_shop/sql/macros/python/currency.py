"""Currency formatting macros."""


def line_total_cents(price_cents: str, quantity: str) -> str:
    """Calculate line total cents from a unit price and quantity."""
    return f"{price_cents} * {quantity}"
