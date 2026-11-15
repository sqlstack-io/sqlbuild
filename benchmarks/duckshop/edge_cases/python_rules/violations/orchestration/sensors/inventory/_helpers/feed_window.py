"""Feed window that reads support private to another domain."""

from ...fulfillment._helpers.hours import CUTOFF_HOURS

FEED_WINDOW_HOURS: int = CUTOFF_HOURS * 2
