"""Write each shipped order as one carrier line."""

from python.utils.currency import amounts


def export_shipments(ctx: object) -> None:
    """Write each shipped order as one carrier line."""
    label: str = "Expédition"
    print(label, amounts.cents_text(1250), ctx)
