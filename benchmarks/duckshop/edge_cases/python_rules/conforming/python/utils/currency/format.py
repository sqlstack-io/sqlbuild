"""Format money for carrier files."""


def cents_text(cents: int) -> str:
    """Return cents as a decimal amount."""
    return f"{cents // 100}.{cents % 100:02d}"
