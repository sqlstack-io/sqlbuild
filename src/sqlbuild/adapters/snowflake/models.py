"""Snowflake adapter models."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal


@dataclass(frozen=True)
class UsageQuery:
    """One access-history read: its SQL and bound parameters."""

    sql: str
    params: tuple[object, ...]


@dataclass(frozen=True)
class CreditPrice:
    """A price per Snowflake credit in USD and where it came from: `rate_sheet` or `list_price`."""

    usd_per_credit: Decimal
    source: str
