"""Shared building blocks for the DuckShop synthetic data generators.

Every generator draws from its own seeded random stream, so the same code version always produces
the same rows, and changing one table's generator does not shift another table's data.
"""

from __future__ import annotations

import random
from dataclasses import dataclass, field
from datetime import UTC, date, datetime, time, timedelta
from zoneinfo import ZoneInfo

SEED = 20260331
"""Fixed seed for every generator. Change it only together with the data version below."""

DATA_VERSION = "1"
"""Bumped whenever generated rows change on purpose."""

VALUATION_DATE = date(2026, 3, 31)
ORDERS_START = date(2026, 1, 1)
CLICKSTREAM_START = date(2026, 3, 9)
UTC = UTC


def rng(*parts: object) -> random.Random:
    """A random stream unique to one generator, derived from the fixed seed."""

    return random.Random(f"duckshop:{SEED}:{DATA_VERSION}:" + ":".join(str(p) for p in parts))


def chance(r: random.Random, probability: float) -> bool:
    return r.random() < probability


def weighted(r: random.Random, options: dict[object, float]) -> object:
    keys = list(options)
    return r.choices(keys, weights=[options[k] for k in keys], k=1)[0]


def utc(value: datetime) -> datetime:
    """A timezone-aware UTC copy of a naive UTC or aware datetime."""

    if value.tzinfo is None:
        return value.replace(tzinfo=UTC)
    return value.astimezone(UTC)


def naive_utc(value: datetime) -> datetime:
    return utc(value).replace(tzinfo=None)


def in_zone(value: datetime, zone: str) -> datetime:
    """The same instant expressed in an IANA zone, keeping its UTC offset (DST-aware)."""

    return utc(value).astimezone(ZoneInfo(zone))


def local_naive(value: datetime, zone: str) -> datetime:
    """Wall-clock time in a zone without offset, as warehouse-local NTZ columns store it."""

    return in_zone(value, zone).replace(tzinfo=None)


def at(day: date, hour: int = 0, minute: int = 0, second: int = 0) -> datetime:
    """A naive UTC datetime on one day."""

    return datetime.combine(day, time(hour, minute, second))


def random_instant(r: random.Random, start: date, end: date) -> datetime:
    """A naive UTC instant uniformly between the start of `start` and the end of `end`."""

    span = (at(end) + timedelta(days=1) - at(start)).total_seconds()
    return at(start) + timedelta(seconds=int(r.random() * span))


def days(start: date, end: date) -> list[date]:
    return [start + timedelta(days=i) for i in range((end - start).days + 1)]


def messy_case(r: random.Random, text: str) -> str:
    """The same text in a random letter case, as hand-keyed source systems produce it."""

    return weighted(r, {text: 6, text.lower(): 2, text.upper(): 1, text.title(): 1})  # type: ignore[return-value]


def messy_sku(r: random.Random, sku: str) -> str:
    """A SKU as an upstream system might send it: lower case, underscores or stray spaces."""

    roll = r.random()
    if roll < 0.75:
        return sku
    if roll < 0.85:
        return sku.lower()
    if roll < 0.93:
        return sku.replace("-", "_", 1)
    return f" {sku} "


@dataclass
class ChangeLog:
    """An append-only change log: every version of a row with `_op`, `_seq` and `_loaded_at`.

    `_seq` follows load order, not event order, so late and out-of-order arrivals look like they do
    in a real replication stream.
    """

    name: str
    _pending: list[tuple[datetime, int, dict[str, object]]] = field(default_factory=list)

    def add(self, row: dict[str, object], loaded_at: datetime, op: str = "I") -> None:
        self._pending.append((naive_utc(loaded_at), len(self._pending), {**row, "_op": op}))

    def insert(self, row: dict[str, object], loaded_at: datetime) -> None:
        self.add(row, loaded_at, "I")

    def update(self, row: dict[str, object], loaded_at: datetime) -> None:
        self.add(row, loaded_at, "U")

    def delete(self, row: dict[str, object], loaded_at: datetime) -> None:
        self.add(row, loaded_at, "D")

    def redeliver(self, row: dict[str, object], loaded_at: datetime, op: str = "I") -> None:
        """The same version delivered twice, as at-least-once replication does."""

        self.add(row, loaded_at, op)

    def rows(self) -> list[dict[str, object]]:
        ordered = sorted(self._pending, key=lambda item: (item[0], item[1]))
        return [
            {**row, "_seq": index + 1, "_loaded_at": loaded_at}
            for index, (loaded_at, _, row) in enumerate(ordered)
        ]


@dataclass
class EventLog:
    """An append-only event table where a few events are delivered twice."""

    name: str
    _rows: list[tuple[datetime, int, dict[str, object]]] = field(default_factory=list)

    def add(self, row: dict[str, object], loaded_at: datetime) -> None:
        self._rows.append(
            (naive_utc(loaded_at), len(self._rows), {**row, "_loaded_at": naive_utc(loaded_at)})
        )

    def rows(self) -> list[dict[str, object]]:
        return [row for _, _, row in sorted(self._rows, key=lambda item: (item[0], item[1]))]


def load_lag(r: random.Random, event_at: datetime, *, late_share: float = 0.04) -> datetime:
    """When a change reaches the warehouse: minutes later, or days later for late arrivals."""

    if chance(r, late_share):
        return naive_utc(event_at) + timedelta(days=r.randint(1, 6), minutes=r.randint(0, 600))
    return naive_utc(event_at) + timedelta(minutes=r.randint(2, 45), seconds=r.randint(0, 59))


def emit(
    log: ChangeLog,
    r: random.Random,
    versions: list[tuple[datetime, dict[str, object]]],
    *,
    late_share: float = 0.04,
    duplicate_share: float = 0.02,
    delete_at: datetime | None = None,
) -> None:
    """Write one entity's versions to a change log: an insert, then updates, maybe a delete.

    Each version reaches the warehouse after a short lag, or days later for a late arrival, so a
    late older version can land after a newer one. A few versions are delivered twice.
    """

    for index, (event_at, row) in enumerate(versions):
        op = "I" if index == 0 else "U"
        loaded_at = load_lag(r, event_at, late_share=late_share)
        log.add(row, loaded_at, op)
        if chance(r, duplicate_share):
            log.redeliver(row, loaded_at + timedelta(minutes=r.randint(1, 90)), op)
    if delete_at is not None:
        log.delete(versions[-1][1], load_lag(r, delete_at, late_share=0))


def money(r: random.Random, low: float, high: float, exponent: int) -> int:
    """A minor-unit amount between two major-unit values for a currency exponent."""

    value = r.uniform(low, high)
    if exponent == 0:
        return int(round(value / 10) * 10)
    minor = int(round(value * 10**exponent))
    return minor - minor % 10 + 9 if exponent == 2 and minor > 100 else minor
