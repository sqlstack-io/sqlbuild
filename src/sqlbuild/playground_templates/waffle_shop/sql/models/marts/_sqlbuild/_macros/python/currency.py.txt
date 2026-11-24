"""Currency formatting macros."""


def cents_to_dollars(column: str) -> str:
    """Convert a cents integer column to a dollars decimal with two decimal places."""
    return f"ROUND(({column}) / 100.0, 2)"
