"""Describe Snowflake cost-collection failures and price credits the project leaves unpriced."""

from __future__ import annotations

import json
from datetime import UTC, datetime
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

from sqlbuild.adapters.snowflake.constants import (
    ACCOUNT_SQL,
    CREDIT_PRICE_CACHE_FILE,
    DEFAULT_LIST_PRICE_EDITION,
    FALLBACK_LIST_PRICE_REGION,
    LIST_PRICE_EDITIONS,
    LIST_PRICE_SOURCE,
    LIST_PRICES,
    RATE_SHEET_CURRENCY,
    RATE_SHEET_SOURCE,
    RATE_SHEET_SQL,
)
from sqlbuild.adapters.snowflake.models import CreditPrice

_FAILURE_DETAIL_LIMIT: int = 200


def cost_failure_cause(error: Exception) -> str:
    """Name a cost-collection failure by type and its message on one bounded line."""

    detail: str = " ".join(str(error).split())
    if not detail:
        return type(error).__name__
    if len(detail) > _FAILURE_DETAIL_LIMIT:
        detail = f"{detail[: _FAILURE_DETAIL_LIMIT - 3]}..."
    return f"{type(error).__name__}: {detail}"


def list_price(*, region: str | None, edition: str = DEFAULT_LIST_PRICE_EDITION) -> CreditPrice:
    """The list price for `region` and `edition`, per Snowflake's price table of 2026-11-20."""

    name: str = (region or "").rsplit(".", 1)[-1].upper()
    prices: tuple[str, str, str] = LIST_PRICES.get(name, LIST_PRICES[FALLBACK_LIST_PRICE_REGION])
    edition_name: str = edition if edition in LIST_PRICE_EDITIONS else DEFAULT_LIST_PRICE_EDITION
    return CreditPrice(Decimal(prices[LIST_PRICE_EDITIONS.index(edition_name)]), LIST_PRICE_SOURCE)


def resolve_credit_price(
    *, connection: Any, cache_dir: Path | None, today: datetime | None = None
) -> CreditPrice:
    """The account's rate-sheet price, else its region's list price; kept for the UTC day."""

    row: tuple[Any, ...] | None = _account(connection=connection)
    account: str = str(row[0]) if row and row[0] else ""
    region: str | None = str(row[1]) if row and len(row) > 1 and row[1] else None
    if not account:
        return _rate_sheet(connection=connection) or list_price(region=region)
    key: str = f"{account}|{(today or datetime.now(UTC)).date().isoformat()}"
    held: CreditPrice | None = _read_cache(cache_dir=cache_dir, key=key)
    if held is not None:
        return held
    price: CreditPrice = _rate_sheet(connection=connection) or list_price(region=region)
    _write_cache(cache_dir=cache_dir, key=key, price=price)
    return price


def _fetchone(*, connection: Any, sql: str) -> tuple[Any, ...] | None:
    cursor: Any = connection.cursor()
    try:
        cursor.execute(sql)
        row: tuple[Any, ...] | None = cursor.fetchone()
        return row
    finally:
        cursor.close()


def _account(*, connection: Any) -> tuple[Any, ...] | None:
    """The account locator and region; none when Snowflake refuses the read."""

    from snowflake.connector.errors import DatabaseError

    try:
        return _fetchone(connection=connection, sql=ACCOUNT_SQL)
    except DatabaseError:
        return None


def _rate_sheet(*, connection: Any) -> CreditPrice | None:
    """The rate sheet's newest USD compute price; none without an organisation role to read it."""

    from snowflake.connector.errors import DatabaseError

    try:
        row: tuple[Any, ...] | None = _fetchone(connection=connection, sql=RATE_SHEET_SQL)
    except DatabaseError:
        return None
    if not row or row[0] is None or str(row[1] or "").upper() != RATE_SHEET_CURRENCY:
        return None
    try:
        rate: Decimal = Decimal(str(row[0]))
    except InvalidOperation:
        return None
    return CreditPrice(rate, RATE_SHEET_SOURCE) if rate > 0 else None


def _read_cache(*, cache_dir: Path | None, key: str) -> CreditPrice | None:
    if cache_dir is None:
        return None
    try:
        entry: Any = json.loads((cache_dir / CREDIT_PRICE_CACHE_FILE).read_text()).get(key)
        return CreditPrice(Decimal(str(entry["usd_per_credit"])), str(entry["source"]))
    except (OSError, ValueError, KeyError, TypeError, AttributeError, InvalidOperation):
        return None


def _write_cache(*, cache_dir: Path | None, key: str, price: CreditPrice) -> None:
    if cache_dir is None:
        return
    entry: dict[str, str] = {"usd_per_credit": str(price.usd_per_credit), "source": price.source}
    try:
        cache_dir.mkdir(parents=True, exist_ok=True)
        (cache_dir / CREDIT_PRICE_CACHE_FILE).write_text(json.dumps({key: entry}))
    except OSError:
        return
