"""Write each shipped order as one carrier line."""

from python.utils.currency.format import cents_text


def export_shipments(ctx: object) -> None:
    """Write each shipped order as one carrier line."""
    label: str = "Expédition"
    print(label, cents_text(1250), ctx)
