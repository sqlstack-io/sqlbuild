"""Counting SQLQuack calls while local work is checked."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

import pytest
import sqlquack

from sqlbuild.executor.local._helpers import conversion as conversion_module
from sqlbuild.executor.local._helpers.conversion import check_all, local_check
from sqlbuild.executor.local.models import LocalWorkCounts

ORDERS_SCHEMA_RELATIONS: dict[str, list[list[str]]] = {
    "SHOP.RAW.ORDERS": [["ID", "NUMBER(38,0)"], ["AMOUNT", "NUMBER(38,0)"]],
    "SHOP.RAW.CUSTOMERS": [["ID", "NUMBER(38,0)"]],
}


def counting_convert(
    monkeypatch: pytest.MonkeyPatch,
) -> list[frozenset[str]]:
    """Replace `sqlquack.convert` with one that records each call's schema relations."""

    calls: list[frozenset[str]] = []
    original: Callable[..., Any] = sqlquack.convert

    def convert(*, sql: str, options: Any, **rest: Any) -> Any:
        calls.append(frozenset(dict(options.schema or {}).get("relations", {})))
        return original(sql=sql, options=options, **rest)

    monkeypatch.setattr(sqlquack, "convert", convert)
    return calls


def check_each(
    *, statements: Sequence[str], schema: dict[str, Any] | None, repeats: int, convert: bool
) -> tuple[int, list[Any]]:
    """How many statements workers checked up front, then each check read `repeats` times."""

    pooled: int = check_all(sqls=list(statements), schema=schema, time_zone=None, convert=convert)
    repeated: list[str] = list(statements) * repeats
    return pooled, [
        local_check(sql=sql, schema=schema, time_zone=None, path="test.sql", convert=convert)
        for sql in repeated
    ]


def work_counts() -> LocalWorkCounts:
    """Conversions, bindings, schema serialisations and cache reads this process did so far."""

    return conversion_module._WORK.counts()


def work_delta(before: LocalWorkCounts) -> LocalWorkCounts:
    """The work done since `before`."""

    after: LocalWorkCounts = work_counts()
    return LocalWorkCounts(
        conversions=after.conversions - before.conversions,
        bindings=after.bindings - before.bindings,
        schema_keys=after.schema_keys - before.schema_keys,
        cached=after.cached - before.cached,
    )


def fresh_run(monkeypatch: pytest.MonkeyPatch) -> None:
    """Forget this process's checks, as a new `sqb` process starts without them."""

    monkeypatch.setattr(
        "sqlbuild.executor.local._helpers.conversion._WORK",
        conversion_module._LocalWork(),
    )
