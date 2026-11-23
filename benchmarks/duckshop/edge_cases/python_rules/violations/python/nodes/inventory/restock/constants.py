"""Restock thresholds per warehouse."""

from python.nodes.inventory.restock._helpers import thresholds

RESTOCK_THRESHOLDS: dict[str, int] = thresholds.load_thresholds()
